use gimli::{
    AttributeValue::{DebugStrRef, Exprloc, LocationListsRef, UnitRef},
    DebuggingInformationEntry, Dwarf, Reader, Unit, UnitOffset, UnitSectionOffset,
};

use anyhow::{anyhow, bail, Result};

use crate::call_stack::MemoryAccess;
use crate::evaluate::attributes;
use crate::evaluate::evaluate;
use crate::evaluate::value_information::ValueInformation;
use crate::evaluate::EvaluatorResult;
use crate::registers::Registers;
use crate::source_information::SourceInformation;
use crate::utils::in_range;

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: Option<String>,
    pub value: String,
    pub type_: Option<String>,
    //    pub locations: Vec<u32>, // u32 or registery number
    pub source: Option<SourceInformation>,
    pub location: Vec<ValueInformation>,
}

impl Variable {
    pub fn get_variable<M: MemoryAccess, R: Reader<Offset = usize>>(
        dwarf: &Dwarf<R>,
        registers: &Registers,
        memory: &mut M,
        section_offset: UnitSectionOffset,
        unit_offset: UnitOffset,
        frame_base: Option<u64>,
        cwd: &str,
    ) -> Result<Variable> {
        // Get the program counter.
        let pc: u32 = *registers
            .get_register_value(
                &(registers.program_counter_register.ok_or(anyhow!(
                    "Requires that the program counter register is known"
                ))? as u16),
            )
            .ok_or(anyhow!(
                "Requies that the program counter registers has a value"
            ))?;

        // Get the variable die.
        let header =
            dwarf
                .debug_info
                .header_from_offset(match section_offset.as_debug_info_offset() {
                    Some(val) => val,
                    None => bail!("Could not convert section offset into debug info offset"),
                })?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let die = unit.entry(unit_offset)?;

        let name = get_var_name(dwarf, &unit, &die)?;

        // Get the source code location the variable was decleard.
        let source = match find_variable_source_information(dwarf, &unit, &die, cwd) {
            Ok(source) => Some(source),
            Err(_) => None,
        };

        let expression = match find_variable_location(dwarf, &unit, &die, pc)? {
            VariableLocation::Expression(expr) => expr,
            VariableLocation::LocationListEntry(llent) => llent.data,
            VariableLocation::OutOfRange => {
                return Ok(Variable {
                    name,
                    value: "<OutOfRange>".to_owned(),
                    type_: None,
                    source,
                    location: vec![],
                });
            }
            VariableLocation::NoLocation => {
                return Ok(Variable {
                    name,
                    value: "<OptimizedOut>".to_owned(),
                    type_: None,
                    source,
                    location: vec![],
                });
            }
        };

        let (type_section_offset, type_unit_offset) = find_variable_type_die(dwarf, &unit, &die)?;
        let header = dwarf.debug_info.header_from_offset(
            match type_section_offset.as_debug_info_offset() {
                Some(val) => val,
                None => bail!("Could not convert the section offset into debug info offset"),
            },
        )?;
        let type_unit = gimli::Unit::new(dwarf, header)?;
        let type_die = unit.entry(type_unit_offset)?;

        match evaluate(
            dwarf,
            &unit,
            pc,
            expression,
            frame_base,
            Some(&type_unit),
            Some(&type_die),
            registers,
            memory,
        )? {
            EvaluatorResult::Complete(val) => Ok(Variable {
                name,
                value: val.to_string(),
                type_: Some(val.get_type()),
                source,
                location: val.get_variable_information(),
            }),
            EvaluatorResult::Requires(_req) => Err(anyhow!("Requires mem or reg")),
        }
    }
}

pub fn is_variable_die<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> bool {
    // Check that it is a variable.
    return die.tag() == gimli::DW_TAG_variable
        || die.tag() == gimli::DW_TAG_formal_parameter
        || die.tag() == gimli::DW_TAG_constant;
}

fn get_var_name<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
) -> Result<Option<String>> {
    if is_variable_die(die) {
        // Get the name of the variable.
        if let Ok(Some(DebugStrRef(offset))) = die.attr_value(gimli::DW_AT_name) {
            return Ok(Some(dwarf.string(offset)?.to_string()?.to_string()));
        } else if let Ok(Some(offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match offset {
                UnitRef(o) => {
                    if let Ok(ndie) = unit.entry(o) {
                        return get_var_name(dwarf, unit, &ndie);
                    }
                }
                _ => {
                    unimplemented!();
                }
            };
        }

        return Ok(None);
    } else {
        return Err(anyhow!("This die is not a variable"));
    }
}

pub enum VariableLocation<R: Reader<Offset = usize>> {
    Expression(gimli::Expression<R>),
    LocationListEntry(gimli::LocationListEntry<R>),
    OutOfRange,
    NoLocation,
}

pub fn find_variable_location<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
    pc: u32,
) -> Result<VariableLocation<R>> {
    if is_variable_die(die) {
        match die.attr_value(gimli::DW_AT_location)? {
            Some(Exprloc(expr)) => {
                return Ok(VariableLocation::Expression(expr));
            }
            Some(LocationListsRef(offset)) => {
                let mut locations = dwarf.locations(unit, offset)?;
                while let Some(llent) = locations.next()? {
                    if in_range(pc, &llent.range) {
                        return Ok(VariableLocation::LocationListEntry(llent));
                    }
                }

                return Ok(VariableLocation::OutOfRange);
            }
            None => return Ok(VariableLocation::NoLocation),
            Some(v) => {
                bail!("Unimplemented for {:?}", v);
            }
        }
    } else {
        return Err(anyhow!("This die is not a variable"));
    }
}

pub fn find_variable_type_die<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
) -> Result<(UnitSectionOffset, UnitOffset)> {
    if is_variable_die(die) {
        match attributes::type_attribute(dwarf, unit, die)? {
            Some(result) => return Ok(result),
            None => {
                if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
                    match die_offset {
                        UnitRef(offset) => {
                            if let Ok(ao_die) = unit.entry(offset) {
                                return find_variable_type_die(dwarf, unit, &ao_die);
                            }
                        }
                        _ => {
                            unimplemented!();
                        }
                    };
                }

                return Err(anyhow!("Could not find this variables type die"));
            }
        }
    } else {
        return Err(anyhow!("This die is not a variable"));
    }
}

pub fn find_variable_source_information<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
    cwd: &str,
) -> Result<SourceInformation> {
    if is_variable_die(die) {
        if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match die_offset {
                UnitRef(offset) => {
                    let ao_die = unit.entry(offset)?;
                    return find_variable_source_information(dwarf, unit, &ao_die, cwd);
                }
                _ => {
                    unimplemented!();
                }
            };
        } else {
            return SourceInformation::get_die_source_information(dwarf, unit, die, cwd);
        }
    } else {
        return Err(anyhow!("This die is not a variable"));
    }
}
