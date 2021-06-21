use crate::debugger::call_stack::CallFrame;
use crate::debugger::die_in_range;
use crate::debugger::EvaluatorValue;
use crate::debugger::check_frame_base;
use crate::debugger::eval_location;
use crate::debugger::get_var_name;
use crate::debugger::source_information::SourceInformation;
use crate::debugger::evaluate::EvaluatorResult;
use crate::debugger::evaluate::EvalResult;
use crate::debugger::variable::VariableCreator;
use crate::debugger::variable::is_variable_die;
use crate::debugger::variable::Variable;

use crate::get_current_unit;

use probe_rs::MemoryInterface;

use gimli::{
    Reader,
    Dwarf,
    Unit,
    DebuggingInformationEntry,
    EntriesTreeNode,
    AttributeValue::DebugStrRef,
    UnitSectionOffset,
    UnitOffset,
};

use anyhow::Result;


#[derive(Debug, Clone)]
pub struct StackFrame {
    pub call_frame: CallFrame,
    pub name: String,
    pub source: SourceInformation,
    pub variables: Vec<Variable>,
}


#[derive(Debug, Clone)]
pub struct StackFrameCreator {
    pub section_offset: gimli::UnitSectionOffset,
    pub unit_offset: gimli::UnitOffset,
    pub dies_to_check: Vec<gimli::UnitOffset>,
    pub call_frame: CallFrame,
    pub name: String,
    pub source: SourceInformation,
    pub frame_base: Option<u64>,
    pub variables: Vec<Variable>, 
}


impl StackFrameCreator {
    pub fn new<R: Reader<Offset = usize>>(call_frame: CallFrame,
                                          dwarf: &Dwarf<R>,
                                          cwd: &str
                                          ) -> Result<StackFrameCreator>
    {
        let (section_offset, unit_offset) = find_function_die(dwarf, call_frame.code_location as u32)?;
        let header = dwarf.debug_info.header_from_offset(section_offset.as_debug_info_offset().unwrap())?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let mut tree = unit.entries_tree(Some(unit_offset))?;
        let node = tree.root()?;

        let name = match node.entry().attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => format!("{:?}", dwarf.string(offset)?.to_string()?),
            _ => "<unknown>".to_string(),
        };

        let source = SourceInformation::get_die_source_information(dwarf, &unit, &node.entry(), cwd)?;

        let mut dies_to_check = get_functions_variables_die_offset(dwarf, section_offset, unit_offset, call_frame.code_location as u32)?;

        Ok(StackFrameCreator {
            section_offset: section_offset,
            unit_offset: unit_offset,
            dies_to_check: dies_to_check,
            call_frame: call_frame.clone(),
            name: name,
            source: source,
            frame_base: None,
            variables: vec!(),
        })
    }


    pub fn continue_creation<R: Reader<Offset = usize>>(&mut self, dwarf: &Dwarf<R>) -> Result<bool> {
        //if self.frame_base.is_none() {
        //    self.frame_base =  
        //}
        unimplemented!();
    }


    pub fn get_stack_frame(&self) -> StackFrame {
        StackFrame {
            call_frame: self.call_frame.clone(),
            name:   self.name.clone(),
            source: self.source.clone(),
            variables: self.variables.clone(),
        }
    }
}


impl StackFrame {
    pub fn create<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                  core:    &mut probe_rs::Core,
                  call_frame: &CallFrame,
                  cwd: &str,
                  ) -> Result<StackFrame>
    {
        let (section_offset, unit_offset) = find_function_die(dwarf, call_frame.code_location as u32)?;
        let header = dwarf.debug_info.header_from_offset(section_offset.as_debug_info_offset().unwrap())?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let die = unit.entry(unit_offset)?;

        let name = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => format!("{:?}", dwarf.string(offset)?.to_string()?),
            _ => "<unknown>".to_string(),
        };

        let mut registers = vec!();
        for i in 0..call_frame.registers.len() {
            match call_frame.registers[i] {
                Some(val) => registers.push((i as u16, val)),
                None => (),
            };
        }

        let variables = get_scope_variables(dwarf, core, &unit, &die, call_frame.code_location as u32, &registers)?;

        Ok(StackFrame{
            call_frame: call_frame.clone(),
            name: name,
            source: SourceInformation::get_die_source_information(dwarf, &unit, &die, cwd)?,
            variables: variables,
        })
    }
}


pub fn find_function_die<'a, R: Reader<Offset = usize>>(dwarf: &'a Dwarf<R>,
                                                        address: u32
                                                        ) -> Result<(gimli::UnitSectionOffset, gimli::UnitOffset)>
{
    let unit = get_current_unit(&dwarf, address)?;
    let mut cursor = unit.entries();

    let mut depth = 0;
    let mut res = None; 
    let mut dies = vec!();

    assert!(cursor.next_dfs().unwrap().is_some());
    while let Some((delta_depth, current)) = cursor.next_dfs()? {
        // Update depth value, and break out of the loop when we
        // return to the original starting position.
        depth += delta_depth;
        if depth <= 0 {
            break;
        }

        match current.tag() {
            gimli::DW_TAG_subprogram | gimli::DW_TAG_inlined_subroutine => {
                if let Some(true) = die_in_range(&dwarf, &unit, current, address) {
                    match res {
                        Some(val) => {
                            if val > depth {
                                res = Some(depth);
                                dies = vec!(current.clone());
                            } else if val == depth {
                                dies.push(current.clone());
                            }
                        },
                        None => {
                            res = Some(depth);
                            dies.push(current.clone());
                        },
                    };
                }
            },
            _ => (),
        }; 
    }

    use crate::debugger::evaluate::attributes::name_attribute;
    for d in &dies {
        println!("die name: {:?}", name_attribute(dwarf, d));
    }
    if dies.len() != 1 {
        panic!("panic here");
    }
    return Ok((unit.header.offset(), dies[0].offset()));
}




pub fn get_scope_variables<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                           core:    &mut probe_rs::Core,
                           unit:    &Unit<R>,
                           die:     &DebuggingInformationEntry<'_, '_, R>,
                           pc:      u32,
                           registers: &Vec<(u16, u32)>,
                           ) -> Result<Vec<Variable>>
{
    let mut variables = vec!();
    let frame_base = check_frame_base(dwarf, core, unit, pc, die, None, registers)?;

    let mut tree = unit.entries_tree(Some(die.offset()))?;
    let node = tree.root()?;

    get_scope_variables_search(dwarf, core, unit, pc, node, frame_base, &mut variables, registers)?;
    return Ok(variables);
}


pub fn get_scope_variables_search<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                  core:             &mut probe_rs::Core,
                                  unit:             &Unit<R>,
                                  pc:               u32,
                                  node:             EntriesTreeNode<R>,
                                  frame_base:       Option<u64>,
                                  variables:        &mut Vec<Variable>,
                                  registers:        &Vec<(u16, u32)>,
                                  ) -> Result<()>
{
    let die = node.entry();
    
    // Check if die in range
    match die_in_range(dwarf, unit, die, pc) {
        Some(false) => return Ok(()),
        _ => (),
    };


    if is_variable_die(&die) {
        variables.push(eval_var(dwarf, core, unit, &die, pc, frame_base, registers)?);
    }

    // Recursively process the children.
    let mut children = node.children();
    while let Some(child) = children.next()? {
        get_scope_variables_search(dwarf, core, unit, pc, child, frame_base, variables, registers)?;
    }
    Ok(())
}


pub fn eval_var<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                  core:             &mut probe_rs::Core,
                                  unit:             &Unit<R>,
                                  die:              &DebuggingInformationEntry<R>,
                                  pc:               u32,
                                  frame_base:       Option<u64>,
                                  registers:        &Vec<(u16, u32)>,
                                  ) -> Result<Variable>
{
    let mut vc = VariableCreator::new(dwarf, unit.header.offset(), die.offset(), registers, frame_base, pc)?;

    loop {
        match vc.continue_create(dwarf)? {
            EvalResult::Complete => break,
            EvalResult::RequiresRegister { register } => {
                panic!("unreachable");
                //let value = core.read_core_reg(register)?;
                //regs.insert(register, value);
            },
            EvalResult::RequiresMemory { address, num_words: _ } => {
                let mut buff = vec![0u32; 1];
                core.read_32(address, &mut buff)?;
                vc.add_to_memory(address, buff[0]);
            },
        };
    }

    vc.get_variable()
}



pub fn get_functions_variables_die_offset<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                                                                     section_offset: UnitSectionOffset,
                                                                     unit_offset: UnitOffset,
                                                                     pc: u32
                                                                     ) -> Result<Vec<UnitOffset>>
{
    fn recursive_offset<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                                                   unit: &Unit<R>,
                                                   node:  EntriesTreeNode<R>,
                                                   pc: u32,
                                                   list: &mut Vec<UnitOffset>
                                                   ) -> Result<()>
    {
        let die = node.entry();

        match die_in_range(&dwarf, unit, die, pc) {
            Some(false) => return Ok(()),
            _ => (),
        };

        match die.tag() {
            gimli::DW_TAG_variable => list.push(die.offset()),
            gimli::DW_TAG_formal_parameter => list.push(die.offset()),
            gimli::DW_TAG_constant => list.push(die.offset()),
            _ => (),
        };

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            recursive_offset(dwarf, unit, child, pc, list)?;
        }

        Ok(())
    }


    let header = dwarf.debug_info.header_from_offset(section_offset.as_debug_info_offset().unwrap())?;
    let unit = gimli::Unit::new(dwarf, header)?;
    let mut tree = unit.entries_tree(Some(unit_offset))?;
    let node = tree.root()?;

    let mut die_offsets = vec!();

    // Recursively process the children.
    let mut children = node.children();
    while let Some(child) = children.next()? {
        recursive_offset(dwarf, &unit, child, pc, &mut die_offsets)?;
    }

    Ok(die_offsets)
}

