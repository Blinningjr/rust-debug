pub mod utils;
pub mod evaluate;
//pub mod stacktrace;
pub mod call_stack;

use probe_rs::MemoryInterface;

use utils::{
    die_in_range,
    in_range,
};


use evaluate::value::{
    EvaluatorValue,
    BaseValue,
};


use anyhow::{
    Result,
    anyhow,
};

use gimli::{
    Unit,
    Dwarf,
    DebuggingInformationEntry,
    AttributeValue::{
        DebugStrRef,
        Exprloc,
        LocationListsRef,
        UnitRef,
    },
    Reader,
    EntriesTreeNode,
    DebugFrame,
};

use super::get_current_unit;





pub struct Debugger<'a, R: Reader<Offset = usize>> {
    pub dwarf:          &'a Dwarf<R>,
    pub debug_frame:    &'a DebugFrame<R>,
    pub breakpoints:    Vec<u32>,
}


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn new(dwarf:       &'a Dwarf<R>,
               debug_frame: &'a DebugFrame<R>,
               ) -> Debugger<'a, R> {
        Debugger{
            dwarf:          dwarf,
            debug_frame:    debug_frame,
            breakpoints:    vec!(),
        }
    }
}


pub fn get_current_stacktrace<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>, debug_frame: & DebugFrame<R>, core: &mut probe_rs::Core, cwd: &str) -> Result<Vec<call_stack::StackFrame>>
{
//    let call_stacktrace = stacktrace::create_call_stacktrace(debug_frame, core)?;

        let pc_reg =   probe_rs::CoreRegisterAddress::from(core.registers().program_counter()).0 as usize;
        let link_reg = probe_rs::CoreRegisterAddress::from(core.registers().return_address()).0 as usize;
    let sp_reg =   probe_rs::CoreRegisterAddress::from(core.registers().stack_pointer()).0 as usize;

    let register_file = core.registers();
   
    let mut regs = [0;16];
    for register in register_file.registers() {
        let value = core.read_core_reg(register)?;
   
        regs[probe_rs::CoreRegisterAddress::from(register).0 as usize] = value;
    }

    let mut csu = call_stack::CallStackUnwinder::new(pc_reg, link_reg, sp_reg, regs);
    loop {
        match csu.unwind(debug_frame)? {
            call_stack::UnwindResult::Complete => break,
            call_stack::UnwindResult::RequiresAddress { address } => {
                let mut buff = vec![0u32; 1];
                core.read_32(address, &mut buff)?;
                csu.add_address(address, buff[0]); 
            },
        }
    }
    let call_stacktrace = csu.get_call_stack();

    let mut stacktrace = vec!();
    for cst in &call_stacktrace {
        stacktrace.push(create_stackframe(dwarf, core, cst, cwd)?);
    }
    Ok(stacktrace)
}


pub fn create_stackframe<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                         core:    &mut probe_rs::Core,
                         call_frame: &call_stack::CallFrame,
                         cwd: &str,
                         ) -> Result<call_stack::StackFrame>
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

    let variables = get_scope_variables(dwarf, core, &unit, &die, call_frame.code_location as u32, &registers)?.iter().map(|(n, v)| (n.clone(), format!("{}", v))).collect();

    Ok(call_stack::StackFrame{
        call_frame: call_frame.clone(),
        name: name,
        source: get_die_source_reference(dwarf, &unit, &die, cwd)?,
        variables: variables,
    })
}


pub fn get_scope_variables<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                           core:    &mut probe_rs::Core,
                           unit:    &Unit<R>,
                           die:     &DebuggingInformationEntry<'_, '_, R>,
                           pc:      u32,
                           registers: &Vec<(u16, u32)>,
                           ) -> Result<Vec<(Option<String>, EvaluatorValue<R>)>>
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
                                  variables:        &mut Vec<(Option<String>, EvaluatorValue<R>)>,
                                  registers:        &Vec<(u16, u32)>,
                                  ) -> Result<()>
{
    let die = node.entry();
    
    // Check if die in range
    match die_in_range(dwarf, unit, die, pc) {
        Some(false) => return Ok(()),
        _ => (),
    };

    match eval_location(dwarf, core, unit, pc, &die, frame_base, registers)? {
        Some(val) => {
            let name = get_var_name(dwarf, unit, pc, die)?; // TODO: get name
            variables.push((name, val));
        },
        None => (),
    };

    // Recursively process the children.
    let mut children = node.children();
    while let Some(child) = children.next()? {
        get_scope_variables_search(dwarf, core, unit, pc, child, frame_base, variables, registers)?;
    }
    Ok(())
}


fn get_var_name<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                  unit:     &Unit<R>,
                  pc:       u32,
                  die:      &DebuggingInformationEntry<R>,
                  ) -> Result<Option<String>>
{
    if die.tag() == gimli::DW_TAG_variable ||
        die.tag() == gimli::DW_TAG_formal_parameter ||
            die.tag() == gimli::DW_TAG_constant { // Check that it is a variable.

        if let Ok(Some(DebugStrRef(offset))) =  die.attr_value(gimli::DW_AT_name) { // Get the name of the variable.
            return Ok(Some(dwarf.string(offset).unwrap().to_string().unwrap().to_string()))

        } else if let Ok(Some(offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match offset {
                UnitRef(o) => {
                    if let Ok(ndie) = unit.entry(o) {
                        return get_var_name(dwarf, unit, pc, &ndie);
                    }
                },
                _ => {
                    println!("{:?}", offset);
                    unimplemented!();
                },
            };
        }
    }
    return Ok(None);
}


pub fn get_die_source_reference<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                                           unit:   &Unit<R>,
                                                           die:    &DebuggingInformationEntry<'_, '_, R>,
                                                           cwd:    &str
                                                           ) -> Result<call_stack::SourceReference>
{
    let (file, directory) = match die.attr_value(gimli::DW_AT_decl_file)? {
        Some(gimli::AttributeValue::FileIndex(v)) => {
            match &unit.line_program {
                Some(lp) => {
                    let header = lp.header();
                    match header.file(v) {
                        Some(file_entry)    => {
                            let (file, directory) = match file_entry.directory(header) {
                                Some(dir_av) => {
                                    let mut dir_raw = dwarf.attr_string(&unit, dir_av)?.to_string()?.to_string();
                                    let file_raw = dwarf.attr_string(&unit, file_entry.path_name())?.to_string()?.to_string();
                                    let file = file_raw.trim_start_matches(&dir_raw).to_string();

                                    if !dir_raw.starts_with("/") {
                                        dir_raw = format!("{}/{}", cwd, dir_raw);
                                    }

                                    (file, Some(dir_raw)) 
                                },
                                None => (dwarf.attr_string(&unit, file_entry.path_name())?.to_string()?.to_string(), None),
                            };

                            (Some(file), directory)
                        },
                        None        => (None, None),
                    }
                },
                None    => (None, None),
            }
        },
        None => (None, None),
        Some(v) => unimplemented!("{:?}", v),
    };

    let line = match die.attr_value(gimli::DW_AT_decl_line)? {
        Some(gimli::AttributeValue::Udata(v)) => Some(v),
        None => None,
        Some(v) => unimplemented!("{:?}", v),
    };

    let column = match die.attr_value(gimli::DW_AT_decl_column)? {
        Some(gimli::AttributeValue::Udata(v)) => Some(v),
        None => None,
        Some(v) => unimplemented!("{:?}", v),
    };
    
    Ok(call_stack::SourceReference {
        directory: directory,
        file: file,
        line: line,
        column: column,
    })
}


pub fn find_variable<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                     core:      &mut probe_rs::Core,
                     unit:      &Unit<R>,
                     pc:        u32,
                     search:    &str
                     ) -> Result<EvaluatorValue<R>>
{
    let mut tree    = unit.entries_tree(None)?;
    let root        = tree.root()?;

    return match process_tree(dwarf, core, unit, pc, root, None, search)? {
        Some(val)   => Ok(val),
        None        => Err(anyhow!("Can't find value")), // TODO: Change to a better error.
    };
}


pub fn process_tree<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>, 
                    core:           &mut probe_rs::Core,
                    unit:           &Unit<R>,
                    pc:             u32,
                    node:           EntriesTreeNode<R>,
                    mut frame_base: Option<u64>,
                    search:         &str
                    ) -> Result<Option<EvaluatorValue<R>>>
{
    let die = node.entry();

    // Check if die in range
    match die_in_range(&dwarf, unit, die, pc) {
        Some(false) => return Ok(None),
        _ => (),
    };

    let registers = get_registers(core)?;
    frame_base = check_frame_base(dwarf, core, unit, pc, &die, frame_base, &registers)?;

    // Check for the searched vairable.
    if check_var_name(dwarf, unit, pc, &die, search) {
        match eval_location(dwarf, core, unit, pc, &die, frame_base, &registers)? {
            Some(val) => return Ok(Some(val)),
            None => (),
        };
    }
    
    // Recursively process the children.
    let mut children = node.children();
    while let Some(child) = children.next()? {
        if let Some(result) = process_tree(dwarf, core, unit, pc, child, frame_base, search)? {
            return Ok(Some(result));
        }
    }
    Ok(None)
}


fn get_registers(core: &mut probe_rs::Core) -> Result<Vec<(u16, u32)>>
{
    let mut registers = vec!();

    let register_file = core.registers();
    for reg in register_file.registers() {
        let value = core.read_core_reg(reg)?;
        registers.push((probe_rs::CoreRegisterAddress::from(reg).0, value));
    }

    Ok(registers)
}


fn eval_location<R: Reader<Offset = usize>>(dwarf:         & Dwarf<R>,
                 core:          &mut probe_rs::Core,
                 unit:          &Unit<R>,
                 pc:            u32,
                 die:           &DebuggingInformationEntry<R>,
                 frame_base:    Option<u64>,
                 registers:     &Vec<(u16, u32)>,
                 ) -> Result<Option<EvaluatorValue<R>>> 
{
    //println!("{:?}", die.attr_value(gimli::DW_AT_const_value));
    match die.attr_value(gimli::DW_AT_const_value)? {
        Some(v) => panic!("const_value: {:#?}", v), // TODO: parse the value of the variable
        None => (),
    };


    //println!("{:?}", die.attr_value(gimli::DW_AT_location));
    match die.attr_value(gimli::DW_AT_location)? {
        Some(Exprloc(expr)) => {
            let value = call_evaluate(dwarf, core, unit, pc, expr, frame_base, unit, die, registers)?;

            return Ok(Some(value));
        },
        Some(LocationListsRef(offset)) => {
            let mut locations = dwarf.locations(unit, offset)?;
            while let Some(llent) = locations.next()? {
                if in_range(pc, &llent.range) {
                    let value = call_evaluate(dwarf, core, unit, pc, llent.data, frame_base, unit, die, registers)?;
                    return Ok(Some(value));
                }
            }

            return Ok(Some(EvaluatorValue::OutOfRange));
        },
        None => return Ok(None), //Err(anyhow!("Expected dwarf location informaiton")),//unimplemented!(), //return Err(Error::Io), // TODO: Better error
        Some(v) => {
            println!("{:?}", v);
            unimplemented!();
        },
    }
}


pub fn check_frame_base<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                                   core:       &mut probe_rs::Core,
                                                   unit:       &Unit<R>,
                                                   pc:         u32,
                                                   die:        &DebuggingInformationEntry<'_, '_, R>,
                                                   frame_base: Option<u64>,
                                                   registers:     &Vec<(u16, u32)>,
                                                   ) -> Result<Option<u64>>
{
    if let Some(val) = die.attr_value(gimli::DW_AT_frame_base)? {
        if let Some(expr) = val.exprloc_value() {
            return Ok(match evaluate::evaluate(dwarf, core, unit, pc, expr, frame_base, None, None, registers) {
                //Ok(EvaluatorValue::Value(BaseValue::U64(v))) => Some(v),
                //Ok(EvaluatorValue::Value(BaseValue::U32(v))) => Some(v as u64),
                Ok(EvaluatorValue::Value(BaseValue::Address32(v))) => Some(v as u64),
                Ok(v) => {
                    println!("{:?}", v);
                    unimplemented!()
                },
                Err(err) => panic!(err),
            });
        } else {
            return Ok(None);
        }
    } else {
        return Ok(frame_base);
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



// Good source: DWARF section 6.2
pub fn find_breakpoint_location<'a, R: Reader<Offset = usize>>(dwarf: &'a Dwarf<R>,
                     cwd: &str,
                     path: &str,
                     line: u64,
                     column: Option<u64>
                     ) -> Result<Option<u64>>
{
    let mut locations = vec!();

    let mut units = dwarf.units();
    while let Some(unit_header) = units.next()? {
        let unit = dwarf.unit(unit_header)?; 

        if let Some(ref line_program) = unit.line_program {
            let lp_header = line_program.header();
            
            for file_entry in lp_header.file_names() {

                let directory = match file_entry.directory(lp_header) {
                    Some(dir_av) => {
                        let dir_raw = dwarf.attr_string(&unit, dir_av)?;
                        dir_raw.to_string()?.to_string()
                    },
                    None => continue,
                };
                
                let file_raw = dwarf.attr_string(&unit, file_entry.path_name())?;
                let mut file_path = format!("{}/{}", directory, file_raw.to_string()?.to_string());

                if !file_path.starts_with("/") { // TODO: Find a better solution
                    file_path = format!("{}/{}", cwd, file_path); 
                }

                if path == &file_path {
                    let mut rows = line_program.clone().rows();
                    while let Some((header, row)) = rows.next_row()? {

                        let file_entry = match row.file(header) {
                            Some(v) => v,
                            None => continue,
                        };

                        let directory = match file_entry.directory(header) {
                            Some(dir_av) => {
                                let dir_raw = dwarf.attr_string(&unit, dir_av)?;
                                dir_raw.to_string()?.to_string()
                            },
                            None => continue,
                        };
                        
                        let file_raw = dwarf.attr_string(&unit, file_entry.path_name())?;
                        let mut file_path = format!("{}/{}", directory, file_raw.to_string()?.to_string());
                        if !file_path.starts_with("/") { // TODO: Find a better solution
                            file_path = format!("{}/{}", cwd, file_path); 
                        }

                        if path == &file_path {
                            if let Some(l) = row.line() {
                                if line == l {
                                    locations.push((row.column(), row.address()));
                                }
                            }
                        }
                    }
                }

            }
        }
    }

    match locations.len() {
        0 => return Ok(None),
        len => {
            let search = match column {
                Some(v) => gimli::ColumnType::Column(v),
                None    => gimli::ColumnType::LeftEdge,
            };

            let mut res = locations[0];
            for i in 1..len {
                if locations[i].0 <= search && locations[i].0 > res.0 {
                    res = locations[i];
                }
            }

            return Ok(Some(res.1));
        },
    };
}


fn check_var_name<'a, R: Reader<Offset = usize>>(dwarf: &'a Dwarf<R>,
                                                 unit:     &Unit<R>,
                                                 pc:       u32,
                                                 die:      &DebuggingInformationEntry<R>,
                                                 search:   &str
                                                 ) -> bool
{
    if die.tag() == gimli::DW_TAG_variable ||
        die.tag() == gimli::DW_TAG_formal_parameter ||
            die.tag() == gimli::DW_TAG_constant { // Check that it is a variable.

        if let Ok(Some(DebugStrRef(offset))) =  die.attr_value(gimli::DW_AT_name) { // Get the name of the variable.
            return dwarf.string(offset).unwrap().to_string().unwrap() == search;// Compare the name of the variable. 

        } else if let Ok(Some(offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match offset {
                UnitRef(o) => {
                    if let Ok(ndie) = unit.entry(o) {
                        return check_var_name(dwarf, unit, pc, &ndie, search);
                    }
                },
                _ => {
                    println!("{:?}", offset);
                    unimplemented!();
                },
            };
        }
    }
    return false;
}


pub fn call_evaluate<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>, core:       &mut probe_rs::Core, nunit:      &Unit<R>, pc:         u32, expr:       gimli::Expression<R>, frame_base: Option<u64>, unit:     &Unit<R>, die: &DebuggingInformationEntry<R>, registers:     &Vec<(u16, u32)>,) -> Result<EvaluatorValue<R>>
{
    if let Ok(Some(tattr)) =  die.attr_value(gimli::DW_AT_type) {
        match tattr {
            gimli::AttributeValue::UnitRef(offset) => {
                let die = unit.entry(offset)?;
                return evaluate::evaluate(dwarf, core, nunit, pc, expr, frame_base, Some(unit), Some(&die), registers);
            },
            gimli::AttributeValue::DebugInfoRef(di_offset) => {
                let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                let mut iter = dwarf.debug_info.units();
                while let Ok(Some(header)) = iter.next() {
                    let unit = dwarf.unit(header).unwrap();
                    if let Some(offset) = offset.to_unit_offset(&unit) {
                        let die = unit.entry(offset)?;
                        return evaluate::evaluate(dwarf, core, nunit, pc, expr, frame_base, Some(&unit), Some(&die), registers);
                    }
                }
                return Err(anyhow!(""));
            },
            _ => return Err(anyhow!("")),
        };
    } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
        match die_offset {
            UnitRef(offset) => {
                if let Ok(ndie) = unit.entry(offset) {
                    return call_evaluate(dwarf, core, nunit, pc, expr, frame_base, unit, &ndie, registers);
                }
            },
            _ => {
                println!("{:?}", die_offset);
                unimplemented!();
            },
        };
    }
    return Err(anyhow!(""));
}


