pub mod memory_and_registers;
pub mod utils;
pub mod evaluate;
pub mod call_stack;
pub mod source_information;
pub mod stack_frame;
pub mod variable;


use crate::rust_debug::memory_and_registers::MemoryAndRegisters;
use crate::rust_debug::evaluate::EvaluatorResult;

use utils::{
    die_in_range,
    in_range,
};

use evaluate::value::{
    EvaluatorValue,
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
        UnitRef,
    },
    Reader,
};


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


pub fn call_evaluate<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                                nunit:      &Unit<R>,
                                                pc:         u32,
                                                expr:       gimli::Expression<R>,
                                                frame_base: Option<u64>,
                                                unit:     &Unit<R>,
                                                die: &DebuggingInformationEntry<R>,
                                                memory_and_registers: &MemoryAndRegisters,
                                                ) -> Result<EvaluatorResult<R>>
{
    if let Ok(Some(tattr)) =  die.attr_value(gimli::DW_AT_type) {
        match tattr {
            gimli::AttributeValue::UnitRef(offset) => {
                let die = unit.entry(offset)?;
                return evaluate::evaluate(dwarf, nunit, pc, expr, frame_base, Some(unit), Some(&die), memory_and_registers);
            },
            gimli::AttributeValue::DebugInfoRef(di_offset) => {
                let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                let mut iter = dwarf.debug_info.units();
                while let Ok(Some(header)) = iter.next() {
                    let unit = dwarf.unit(header).unwrap();
                    if let Some(offset) = offset.to_unit_offset(&unit) {
                        let die = unit.entry(offset)?;
                        return evaluate::evaluate(dwarf, nunit, pc, expr, frame_base, Some(&unit), Some(&die), memory_and_registers);
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
                    return call_evaluate(dwarf, nunit, pc, expr, frame_base, unit, &ndie, memory_and_registers);
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


