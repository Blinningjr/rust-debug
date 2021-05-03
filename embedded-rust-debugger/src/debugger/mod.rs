pub mod utils;
pub mod print_dwarf;
pub mod evaluate;
pub mod types;
//pub mod type_value;
pub mod attributes;
pub mod stacktrace;


use utils::{
    die_in_range,
    in_range,
};
use crate::debugger::types::DebuggerType;


use evaluate::value::{
    EvaluatorValue,
    BaseValue,
};


use probe_rs::{
    Core,
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
    UnwindSection,
};

use super::get_current_unit;





pub struct Debugger<R: Reader<Offset = usize>> {
    pub dwarf:          Dwarf<R>,
    pub debug_frame:    DebugFrame<R>,
    pub breakpoints:    Vec<u32>,
}


impl<R: Reader<Offset = usize>> Debugger<R> {
    pub fn new(dwarf:       Dwarf<R>,
               debug_frame: DebugFrame<R>,
               ) -> Debugger<R> {
        Debugger{
            dwarf:          dwarf,
            debug_frame:    debug_frame,
            breakpoints:    vec!(),
        }
    }


    pub fn get_current_stacktrace(&mut self, core: &mut probe_rs::Core) -> Result<Vec<stacktrace::StackFrame>>
    {
        let call_stacktrace = stacktrace::create_call_stacktrace(self, core)?;
        let mut stacktrace = vec!();
        for cst in &call_stacktrace {
            stacktrace.push(self.create_stackframe(cst)?);
        }
        Ok(stacktrace)
    }


    // Good source: DWARF section 6.2
    // TODO: Create a function that correcly gets the directory and file_name;
    pub fn find_location(&mut self,
                         path: &str,
                         line: u64,
                         column: Option<u64>
                         ) -> Result<Option<u64>>
    {
        let mut locations = vec!();

        let mut units = self.dwarf.units();
        while let Some(unit_header) = units.next()? {
            let unit = self.dwarf.unit(unit_header)?; 

            if let Some(ref line_program) = unit.line_program {
                let lp_header = line_program.header();
                
                for file_entry in lp_header.file_names() {

                    let directory = match file_entry.directory(lp_header) {
                        Some(dir_av) => {
                            let dir_raw = self.dwarf.attr_string(&unit, dir_av)?;
                            dir_raw.to_string()?.to_string()
                        },
                        None => continue,
                    };
                    
                    let file_raw = self.dwarf.attr_string(&unit, file_entry.path_name())?;
                    let file_path = format!("{}/{}", directory, file_raw.to_string()?.to_string());

                    if path == &file_path {
                        let mut rows = line_program.clone().rows();
                        while let Some((header, row)) = rows.next_row()? {

                            let file_entry = match row.file(header) {
                                Some(v) => v,
                                None => continue,
                            };

                            let directory = match file_entry.directory(header) {
                                Some(dir_av) => {
                                    let dir_raw = self.dwarf.attr_string(&unit, dir_av)?;
                                    dir_raw.to_string()?.to_string()
                                },
                                None => continue,
                            };
                            
                            let file_raw = self.dwarf.attr_string(&unit, file_entry.path_name())?;
                            let file_path = format!("{}/{}", directory, file_raw.to_string()?.to_string());

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


    pub fn create_stackframe(&mut self,
                             call_frame: &stacktrace::CallFrame
                             ) -> Result<stacktrace::StackFrame>
    {
        let (section_offset, unit_offset) = self.find_function_die(call_frame.code_location as u32)?;
        let header = self.dwarf.debug_info.header_from_offset(section_offset.as_debug_info_offset().unwrap())?;
        let unit = gimli::Unit::new(&self.dwarf, header)?;
        let die = unit.entry(unit_offset)?;

        let name = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => format!("{:?}", self.dwarf.string(offset)?.to_string()?),
            _ => "<unknown>".to_string(),
        };


        Ok(stacktrace::StackFrame{
            call_frame: call_frame.clone(),
            name: name,
            source: self.get_die_source_reference(&unit, &die)?,
        })
    }


    // TODO: Create a function that correcly gets the directory and file_name;
    pub fn get_die_source_reference(&mut self,
                                unit:   &Unit<R>,
                                die:    &DebuggingInformationEntry<'_, '_, R>
                                ) -> Result<stacktrace::SourceReference>
    {
        let (file, directory) = match die.attr_value(gimli::DW_AT_decl_file)? {
            Some(gimli::AttributeValue::FileIndex(v)) => {
                match &unit.line_program {
                    Some(lp) => {
                        let header = lp.header();
                        match header.file(v) {
                            Some(file_entry)    => {
                                let directory = match file_entry.directory(header) {
                                    Some(dir_av) => {
                                        let dir_raw = self.dwarf.attr_string(&unit, dir_av)?;
                                        Some(dir_raw.to_string()?.to_string()) 
                                    },
                                    None => None,
                                };

                                let file_raw = self.dwarf.attr_string(&unit, file_entry.path_name())?;
                                (Some(file_raw.to_string()?.to_string()), directory)
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

        Ok(stacktrace::SourceReference {
            directory: directory,
            file: file,
            line: line,
            column: column,
        })
    }

    pub fn find_function_die(&mut self, address: u32) -> Result<(gimli::UnitSectionOffset, gimli::UnitOffset)> {
        let unit = get_current_unit(&self.dwarf, address)?;
        let mut cursor = unit.entries();

        let mut depth = 0;
        let mut res = None; 
        let mut die = None;

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
                    if let Some(true) = die_in_range(&self.dwarf, &unit, current, address) {
                        match res {
                            Some(val) => {
                                if val > depth {
                                    res = Some(depth);
                                    die = Some(current.clone());
                                } else if val == depth {
                                    panic!("multiple");
                                }
                            },
                            None => {
                                res = Some(depth);
                                die = Some(current.clone());
                            },
                        };
                    }
                },
                _ => (),
            }; 
        }

        match die {
            Some(d) => {
                return Ok((unit.header.offset(), d.offset()));
            },
            None => {
                return Err(anyhow!("Could not find function for address {}", address));
            },
        };
    }


    pub fn find_variable(&mut self,
                         core:      &mut probe_rs::Core,
                         unit:      &Unit<R>,
                         pc:        u32,
                         search:    &str
                         ) -> Result<EvaluatorValue<R>>
    {
        let mut tree    = unit.entries_tree(None)?;
        let root        = tree.root()?;

//        self.print_tree(root)?;
//        unimplemented!();

        return match self.process_tree(core, unit, pc, root, None, search)? {
            Some(val)   => Ok(val),
            None        => Err(anyhow!("Can't find value")), // TODO: Change to a better error.
        };
    }


    pub fn process_tree(&mut self, 
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
        match die_in_range(&self.dwarf, unit, die, pc) {
            Some(false) => return Ok(None),
            _ => (),
        };

        frame_base = self.check_frame_base(core, unit, pc, &die, frame_base)?;

        // Check for the searched vairable.
        if self.check_var_name(unit, pc, &die, search) {
            match self.eval_location(core, unit, pc, &die, frame_base) {
                Ok(v) => return Ok(v),
                Err(_) => (),
            };
        }
        
//        //self.print_die(&die)?;
//        if let Some(dtype) = self.get_var_type(unit, pc, &die) {
//            //println!("{:#?}", dtype);
//            //self.print_die(&die)?;
//            self.eval_location(unit, pc, &die, &dtype, frame_base);
//        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            if let Some(result) = self.process_tree(core, unit, pc, child, frame_base, search)? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }


    fn check_var_name(&mut self,
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
                return self.dwarf.string(offset).unwrap().to_string().unwrap() == search;// Compare the name of the variable. 

            } else if let Ok(Some(offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
                match offset {
                    UnitRef(o) => {
                        if let Ok(ndie) = unit.entry(o) {
                            return self.check_var_name(unit, pc, &ndie, search);
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


    fn get_var_type(&mut self,
                      unit:     &Unit<R>,
                      pc:       u32,
                    die: &DebuggingInformationEntry<R>
                    ) -> Option<DebuggerType>
    {
        if let Ok(Some(tattr)) =  die.attr_value(gimli::DW_AT_type) {
            return match self.parse_type_attr(unit, pc, tattr) {
                Ok(t) => Some(t),
                Err(_) => None,
            };
        } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match die_offset {
                UnitRef(offset) => {
                    if let Ok(ndie) = unit.entry(offset) {
                        return self.get_var_type(unit, pc, &ndie);
                    }
                },
                _ => {
                    println!("{:?}", die_offset);
                    unimplemented!();
                },
            };
        }
        return None;
    }


    fn call_evaluate(&mut self,
                    core:       &mut probe_rs::Core,
                    nunit:      &Unit<R>,
                    pc:         u32,
                    expr:       gimli::Expression<R>,
                    frame_base: Option<u64>,
                    unit:     &Unit<R>,
                    die: &DebuggingInformationEntry<R>
                    ) -> Result<EvaluatorValue<R>>
    {
        if let Ok(Some(tattr)) =  die.attr_value(gimli::DW_AT_type) {
            match die.attr_value(gimli::DW_AT_type)? {
                Some(gimli::AttributeValue::UnitRef(offset)) => {
                    let die = unit.entry(offset)?;
                    return self.evaluate(core, nunit, pc, expr, frame_base, Some(unit), Some(&die));
                },
                Some(gimli::AttributeValue::DebugInfoRef(di_offset)) => {
                    let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                    let mut iter = self.dwarf.debug_info.units();
                    while let Ok(Some(header)) = iter.next() {
                        let unit = self.dwarf.unit(header).unwrap();
                        if let Some(offset) = offset.to_unit_offset(&unit) {
                            let die = unit.entry(offset)?;
                            return self.evaluate(core, nunit, pc, expr, frame_base, Some(&unit), Some(&die));
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
                        return self.call_evaluate(core, nunit, pc, expr, frame_base, unit, &ndie);
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

    fn eval_location(&mut self,
                     core:              &mut probe_rs::Core,
                     unit:              &Unit<R>,
                     pc:                u32,
                     die:               &DebuggingInformationEntry<R>,
                     frame_base:        Option<u64>
                     ) -> Result<Option<EvaluatorValue<R>>> 
    {
        //println!("{:?}", die.attr_value(gimli::DW_AT_location));
        match die.attr_value(gimli::DW_AT_location)? {
            Some(Exprloc(expr)) => {
                self.print_die(&die)?;
                let value = self.call_evaluate(core, unit, pc, expr, frame_base, unit, die)?;
                println!("\n");

                return Ok(Some(value));
            },
            Some(LocationListsRef(offset)) => {
                self.print_die(&die)?;
                let mut locations = self.dwarf.locations(unit, offset)?;
                while let Some(llent) = locations.next()? {
                    if in_range(pc, &llent.range) {
                        let value = self.call_evaluate(core, unit, pc, llent.data, frame_base, unit, die)?;
                        return Ok(Some(value));
                    }
                }

                return Ok(Some(EvaluatorValue::OutOfRange));
            },
            None => return Err(anyhow!("Expected dwarf location informaiton")),//unimplemented!(), //return Err(Error::Io), // TODO: Better error
            Some(v) => {
                println!("{:?}", v);
                unimplemented!();
            },
        }
    }
    

    pub fn check_frame_base(&mut self,
                            core:       &mut probe_rs::Core,
                            unit:       &Unit<R>,
                            pc:         u32,
                            die:        &DebuggingInformationEntry<'_, '_, R>,
                            frame_base: Option<u64>
                            ) -> Result<Option<u64>>
    {
        if let Some(val) = die.attr_value(gimli::DW_AT_frame_base)? {
            if let Some(expr) = val.exprloc_value() {
                return Ok(match self.evaluate(core, unit, pc, expr, frame_base, None, None) {
                    Ok(EvaluatorValue::Value(BaseValue::U64(v))) => Some(v),
                    Ok(EvaluatorValue::Value(BaseValue::U32(v))) => Some(v as u64),
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
}

