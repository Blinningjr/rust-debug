use crate::call_stack::CallFrame;
use crate::call_stack::MemoryAccess;
use crate::evaluate::evaluate;
use crate::evaluate::value::BaseValue;
use crate::evaluate::EvalResult;
use crate::evaluate::EvaluatorResult;
use crate::evaluate::EvaluatorValue;
use crate::registers::Registers;
use crate::source_information::SourceInformation;
use crate::utils::die_in_range;
use crate::utils::get_current_unit;
use crate::variable::is_variable_die;
use crate::variable::Variable;
use crate::variable::VariableCreator;

use gimli::{
    AttributeValue::DebugStrRef, DebuggingInformationEntry, Dwarf, EntriesTreeNode, Reader, Unit,
    UnitOffset, UnitSectionOffset,
};

use anyhow::{anyhow, bail, Result};

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub call_frame: CallFrame,
    pub name: String,
    pub source: SourceInformation,
    pub variables: Vec<Variable>,
}

impl StackFrame {
    pub fn find_variable(&self, name: &str) -> Option<&Variable> {
        for v in &self.variables {
            match &v.name {
                Some(var_name) => {
                    if var_name == name {
                        return Some(v);
                    }
                }
                None => (),
            };
        }

        return None;
    }
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
    pub fn new<R: Reader<Offset = usize>>(
        call_frame: CallFrame,
        dwarf: &Dwarf<R>,
        cwd: &str,
    ) -> Result<StackFrameCreator> {
        let (section_offset, unit_offset) =
            find_function_die(dwarf, call_frame.code_location as u32)?;
        let header =
            dwarf
                .debug_info
                .header_from_offset(match section_offset.as_debug_info_offset() {
                    Some(val) => val,
                    None => bail!("Could not convert section offset to debug info offset"),
                })?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let mut tree = unit.entries_tree(Some(unit_offset))?;
        let node = tree.root()?;

        let name = match node.entry().attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => format!("{:?}", dwarf.string(offset)?.to_string()?),
            _ => "<unknown>".to_string(),
        };

        let source =
            SourceInformation::get_die_source_information(dwarf, &unit, &node.entry(), cwd)?;

        let dies_to_check = get_functions_variables_die_offset(
            dwarf,
            section_offset,
            unit_offset,
            call_frame.code_location as u32,
        )?;

        Ok(StackFrameCreator {
            section_offset: section_offset,
            unit_offset: unit_offset,
            dies_to_check: dies_to_check,
            call_frame: call_frame.clone(),
            name: name,
            source: source,
            frame_base: None,
            variables: vec![],
        })
    }

    pub fn continue_creation<R: Reader<Offset = usize>, T: MemoryAccess>(
        &mut self,
        dwarf: &Dwarf<R>,
        registers: &mut Registers,
        mem: &mut T,
        cwd: &str,
    ) -> Result<EvalResult> {
        let pc = self.call_frame.code_location as u32;

        registers.stash_registers();
        for i in 0..self.call_frame.registers.len() {
            match self.call_frame.registers[i] {
                Some(val) => registers.add_register_value(i as u16, val),
                None => (),
            };
        }

        if self.frame_base.is_none() {
            let header = match dwarf.debug_info.header_from_offset(
                match self.section_offset.as_debug_info_offset() {
                    Some(val) => val,
                    None => bail!("Could not convert section offset to debug info offset"),
                },
            ) {
                Ok(val) => val,
                Err(err) => {
                    registers.pop_stashed_registers();
                    return Err(anyhow!(err));
                }
            };
            let unit = match gimli::Unit::new(dwarf, header) {
                Ok(val) => val,
                Err(err) => {
                    registers.pop_stashed_registers();
                    return Err(anyhow!(err));
                }
            };

            let die = match unit.entry(self.unit_offset) {
                Ok(val) => val,
                Err(err) => {
                    registers.pop_stashed_registers();
                    return Err(anyhow!(err));
                }
            };

            self.frame_base = match evaluate_frame_base(dwarf, &unit, pc, &die, registers, mem) {
                Ok(FrameBaseResult::Complete(val)) => Some(val),
                Ok(FrameBaseResult::Requires(req)) => return Ok(req),
                Err(err) => {
                    registers.pop_stashed_registers();
                    return Err(err);
                }
            };
        }

        while self.dies_to_check.len() > 0 {
            match self.evaluate_variable(dwarf, registers, mem, pc, cwd) {
                Ok(result) => {
                    match result {
                        EvalResult::Complete => continue,
                        _ => {
                            registers.pop_stashed_registers();
                            return Ok(result);
                        }
                    };
                }
                Err(err) => {
                    registers.pop_stashed_registers();
                    return Err(err);
                }
            };
        }
        registers.pop_stashed_registers();

        Ok(EvalResult::Complete)
    }

    fn evaluate_variable<R: Reader<Offset = usize>, T: MemoryAccess>(
        &mut self,
        dwarf: &Dwarf<R>,
        registers: &mut Registers,
        mem: &mut T,
        pc: u32,
        cwd: &str,
    ) -> Result<EvalResult> {
        let mut vc = VariableCreator::new(
            dwarf,
            self.section_offset,
            self.dies_to_check[0],
            self.frame_base,
            pc,
            cwd,
        )?;

        let result = vc.continue_create(dwarf, registers, mem)?;
        match result {
            EvalResult::Complete => (),
            _ => return Ok(result),
        };

        self.dies_to_check.remove(0);
        self.variables.push(vc.get_variable()?);

        Ok(EvalResult::Complete)
    }

    pub fn get_stack_frame(&self) -> StackFrame {
        StackFrame {
            call_frame: self.call_frame.clone(),
            name: self.name.clone(),
            source: self.source.clone(),
            variables: self.variables.clone(),
        }
    }
}

pub fn find_function_die<'a, R: Reader<Offset = usize>>(
    dwarf: &'a Dwarf<R>,
    address: u32,
) -> Result<(gimli::UnitSectionOffset, gimli::UnitOffset)> {
    let unit = get_current_unit(&dwarf, address)?;
    let mut cursor = unit.entries();

    let mut depth = 0;
    let mut res = None;
    let mut dies = vec![];

    assert!(cursor.next_dfs()?.is_some());
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
                                dies = vec![current.clone()];
                            } else if val == depth {
                                dies.push(current.clone());
                            }
                        }
                        None => {
                            res = Some(depth);
                            dies.push(current.clone());
                        }
                    };
                }
            }
            _ => (),
        };
    }

    if dies.len() != 1 {
        unreachable!();
    }
    return Ok((unit.header.offset(), dies[0].offset()));
}

pub fn get_functions_variables_die_offset<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    section_offset: UnitSectionOffset,
    unit_offset: UnitOffset,
    pc: u32,
) -> Result<Vec<UnitOffset>> {
    fn recursive_offset<R: Reader<Offset = usize>>(
        dwarf: &Dwarf<R>,
        unit: &Unit<R>,
        node: EntriesTreeNode<R>,
        pc: u32,
        list: &mut Vec<UnitOffset>,
    ) -> Result<()> {
        let die = node.entry();

        match die_in_range(dwarf, unit, die, pc) {
            Some(false) => return Ok(()),
            _ => (),
        };

        if is_variable_die(&die) {
            list.push(die.offset());
        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            recursive_offset(dwarf, unit, child, pc, list)?;
        }

        Ok(())
    }

    let header =
        dwarf
            .debug_info
            .header_from_offset(match section_offset.as_debug_info_offset() {
                Some(val) => val,
                None => bail!("Could not convert section offset to debug info offset"),
            })?;
    let unit = gimli::Unit::new(dwarf, header)?;
    let mut tree = unit.entries_tree(Some(unit_offset))?;
    let node = tree.root()?;

    let mut die_offsets = vec![];

    // Recursively process the children.
    let mut children = node.children();
    while let Some(child) = children.next()? {
        recursive_offset(dwarf, &unit, child, pc, &mut die_offsets)?;
    }

    Ok(die_offsets)
}

pub enum FrameBaseResult {
    Complete(u64),
    Requires(EvalResult),
}

pub fn evaluate_frame_base<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    pc: u32,
    die: &DebuggingInformationEntry<'_, '_, R>,
    registers: &mut Registers,
    mem: &mut T,
) -> Result<FrameBaseResult> {
    if let Some(val) = die.attr_value(gimli::DW_AT_frame_base)? {
        if let Some(expr) = val.exprloc_value() {
            let result = evaluate(
                dwarf,
                unit,
                pc,
                expr.clone(),
                None,
                None,
                None,
                registers,
                mem,
            )?;
            let value = match result {
                EvaluatorResult::Complete(val) => val,
                EvaluatorResult::Requires(req) => return Ok(FrameBaseResult::Requires(req)),
            };

            match value {
                EvaluatorValue::Value(BaseValue::Address32(v), _) => {
                    return Ok(FrameBaseResult::Complete(v as u64))
                }
                _ => {
                    unreachable!();
                }
            };
        } else {
            unimplemented!();
        }
    } else {
        return Err(anyhow!("Die has no DW_AT_frame_base attribute"));
    }
}
