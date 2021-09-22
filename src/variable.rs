use gimli::{
    AttributeValue::{DebugStrRef, Exprloc, LocationListsRef, UnitRef},
    DebuggingInformationEntry, Dwarf, Reader, Unit, UnitOffset, UnitSectionOffset,
};

use anyhow::{anyhow, bail, Result};

use crate::call_stack::MemoryAccess;
use crate::evaluate::attributes;
use crate::evaluate::evaluate;
use crate::registers::Registers;
use crate::source_information::SourceInformation;
use crate::utils::in_range;
use crate::variable::evaluate::EvaluatorValue;

/// Defines what debug information a variable has.
#[derive(Debug, Clone)]
pub struct Variable<R: Reader<Offset = usize>> {
    /// The name of the variable.
    pub name: Option<String>,

    /// A tree strucured like the variable type in DWARF, but it also contains the values
    pub value: EvaluatorValue<R>,

    /// The source code location where the variable was declared.
    pub source: Option<SourceInformation>,
}

impl<R: Reader<Offset = usize>> Variable<R> {
    /// Retrieve the variables debug information.
    ///
    /// Description:
    ///
    /// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
    /// * `registers` - A reference to the `Registers` struct.
    /// * `memory` - A reference to a struct that implements the `MemoryAccess` trait.
    /// * `section_offset` - A offset to the compilation unit where the DIE for the variable is
    /// located.
    /// * `unit_offset` - A offset into the compilation unit where the DIE representing the
    /// variable is located.
    /// * `frame_base` - The value of the frame base, which is often needed to evaluate the
    /// variable.
    /// * `cwd` - The work directory of the program being debugged.
    ///
    /// This function will go through the DIE in the compilation unit to find the necessary
    /// debug information.
    /// Then it will use that information to evaluate the value of the variable.
    pub fn get_variable<M: MemoryAccess>(
        dwarf: &Dwarf<R>,
        registers: &Registers,
        memory: &mut M,
        section_offset: UnitSectionOffset,
        unit_offset: UnitOffset,
        frame_base: Option<u64>,
        cwd: &str,
    ) -> Result<Variable<R>> {
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
            VariableLocation::LocationOutOfRange => {
                return Ok(Variable {
                    name,
                    value: EvaluatorValue::LocationOutOfRange,
                    source,
                });
            }
            VariableLocation::NoLocation => {
                return Ok(Variable {
                    name,
                    value: EvaluatorValue::OptimizedOut,
                    source,
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

        let value = evaluate(
            dwarf,
            &unit,
            pc,
            expression,
            frame_base,
            Some(&type_unit),
            Some(&type_die),
            registers,
            memory,
        )?;

        Ok(Variable {
            name,
            value,
            source,
        })
    }
}

/// Will check if the given DIE has one of the DWARF tags that represents a variable.
///
/// Description:
///
/// * `die` - A reference to DIE.
///
/// Will check if the given type has one of the following tags:
/// - DW_TAG_variable
/// - DW_TAG_formal_parameter
/// - DW_TAG_constant
/// If the DIE has one of the tags the function will return `true`, otherwise `false`.
pub fn is_variable_die<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> bool {
    // Check that it is a variable.
    return die.tag() == gimli::DW_TAG_variable
        || die.tag() == gimli::DW_TAG_formal_parameter
        || die.tag() == gimli::DW_TAG_constant;
}

/// Will retrieve the name of a variable DIE.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A reference to gimli-rs `Unit` struct, which the given DIE is located in.
/// * `die` - A reference to DIE.
///
/// Will check if the given DIE represents a variable, if it does not it will return a error.
/// After that it will try to evaluate the `DW_AT_name` attribute and return the result.
/// But if it dose not have the name attribute it will try to get the name from the DIE in the
/// `DW_AT_abstract_origin` attribute.
/// If that attribute is missing it will return `Ok(None)`, because the variable does not have a
/// name.
pub fn get_var_name<R: Reader<Offset = usize>>(
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

/// Holds the location of a variable.
pub enum VariableLocation<R: Reader<Offset = usize>> {
    /// The gimli-rs expression that describes the location of the variable.
    Expression(gimli::Expression<R>),

    /// The gimli-rs location list entry that describes the location of the Variable.
    LocationListEntry(gimli::LocationListEntry<R>),

    /// The variable has no location currently but had or will have one. Note that the location can
    /// be a constant stored in the DWARF stack.
    LocationOutOfRange,

    /// The variable has no location.
    NoLocation,
}

/// Find the location of a variable.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A reference to gimli-rs `Unit` struct which contains the given DIE.
/// * `die` - A reference to the variables DIE that contains the location.
/// * `address` - A address that will be used to find the location, this is most often the current machine code address.
///
/// Will get the location for the given address from the attribute `DW_AT_location` in the variable DIE.
pub fn find_variable_location<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
    address: u32,
) -> Result<VariableLocation<R>> {
    if is_variable_die(die) {
        match die.attr_value(gimli::DW_AT_location)? {
            Some(Exprloc(expr)) => {
                return Ok(VariableLocation::Expression(expr));
            }
            Some(LocationListsRef(offset)) => {
                let mut locations = dwarf.locations(unit, offset)?;
                let mut count = 0;
                while let Some(llent) = locations.next()? {
                    if in_range(address, &llent.range) {
                        return Ok(VariableLocation::LocationListEntry(llent));
                    }
                    count += 1;
                }

                if count > 0 {
                    return Ok(VariableLocation::LocationOutOfRange);
                } else {
                    return Ok(VariableLocation::NoLocation);
                }
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

/// Find the DIE representing the type of a variable.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A reference to gimli-rs `Unit` struct, which the given DIE is located in.
/// * `die` - A reference to the DIE representing a variable, which the resulting type DIE will represent the type off..
///
/// Will find the DIE representing the type of the variable that the given DIE represents.
/// The type DIE is found using the attribute `DW_AT_type` in the given DIE or in the DIE from the
/// attribute `DW_AT_abstract_origin`.
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

/// Retrieve the variables source location where it was declared.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A reference to gimli-rs `Unit` struct, which the given DIE is located in.
/// * `die` - A reference to DIE.
/// * `cwd` - The work directory of the debugged program.
///
/// This function will retrieve the source code location where the variable was declared.
/// The information is retrieved from the  attributes starting with `DW_AT_decl_` in the given DIE,
/// or in the DIE found in the attribute `DW_AT_abstract_origin`.
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
