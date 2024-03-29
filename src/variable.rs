use gimli::{
    AttributeValue::{DebugInfoRef, DebugStrRef, Exprloc, LocationListsRef, UnitRef},
    DebuggingInformationEntry, Dwarf, Reader, Unit, UnitOffset, UnitSectionOffset,
};

use crate::evaluate::attributes;
use crate::evaluate::evaluate;
use crate::registers::Registers;
use crate::source_information::SourceInformation;
use crate::utils::in_range;
use crate::variable::evaluate::EvaluatorValue;
use crate::{call_stack::MemoryAccess, utils::DwarfOffset};
use anyhow::{anyhow, Result};
use log::{error, info, trace};

/// Defines what debug information a variable has.
#[derive(Debug, Clone)]
pub struct Variable<R: Reader<Offset = usize>> {
    /// The name of the variable.
    pub name: Option<String>,

    /// A tree structured like the variable type in DWARF, but it also contains the values
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
        dwarf_offset: DwarfOffset,
        frame_base: Option<u64>,
        cwd: &str,
    ) -> Result<Variable<R>> {
        // Get the program counter.
        let pc: u32 = *registers
            .get_register_value(
                &(registers
                    .program_counter_register
                    .ok_or_else(|| anyhow!("Requires that the program counter register is known"))?
                    as u16),
            )
            .ok_or_else(|| anyhow!("Requires that the program counter registers has a value"))?;

        // Get the variable die.
        let header = dwarf.debug_info.header_from_offset(
            match dwarf_offset.section_offset.as_debug_info_offset() {
                Some(val) => val,
                None => {
                    error!("Could not convert section offset into debug info offset");
                    return Err(anyhow!(
                        "Could not convert section offset into debug info offset"
                    ));
                }
            },
        )?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let die = unit.entry(dwarf_offset.unit_offset)?;

        let name = get_var_name(dwarf, &unit, &die)?;
        info!("name: {:?}", name);

        // Get the source code location the variable was declared.
        let source = match find_variable_source_information(dwarf, &unit, &die, cwd) {
            Ok(source) => Some(source),
            Err(_) => None,
        };
        info!("has source");

        let expression = match find_variable_location(dwarf, &unit, &die, pc)? {
            VariableLocation::Expression(expr) => {
                trace!("VariableLocation::Expression");
                expr
            }
            VariableLocation::LocationListEntry(llent) => {
                trace!("VariableLocation::LocationListEntry");
                llent.data
            }
            VariableLocation::LocationOutOfRange => {
                trace!("VariableLocation::LocationOutOfRange");
                return Ok(Variable {
                    name,
                    value: EvaluatorValue::LocationOutOfRange,
                    source,
                });
            }
            VariableLocation::NoLocation => {
                trace!("VariableLocation::NoLocation");
                return Ok(Variable {
                    name,
                    value: EvaluatorValue::OptimizedOut,
                    source,
                });
            }
        };
        info!("has expression");

        let (type_section_offset, type_unit_offset) = find_variable_type_die(dwarf, &unit, &die)?;
        info!("type sec offset: {:?}", type_section_offset);
        info!("type unit offset: {:?}", type_unit_offset);
        let header = dwarf.debug_info.header_from_offset(
            match type_section_offset.as_debug_info_offset() {
                Some(val) => val,
                None => {
                    error!("Could not convert section offset into debug info offset");
                    return Err(anyhow!(
                        "Could not convert section offset into debug info offset"
                    ));
                }
            },
        )?;
        let type_unit = gimli::Unit::new(dwarf, header)?;
        let type_die = type_unit.entry(type_unit_offset)?;

        info!("has type");

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
    die.tag() == gimli::DW_TAG_variable
        || die.tag() == gimli::DW_TAG_formal_parameter
        || die.tag() == gimli::DW_TAG_constant
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
                DebugInfoRef(di_offset) => {
                    let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                    let mut iter = dwarf.debug_info.units();
                    while let Ok(Some(header)) = iter.next() {
                        let unit = dwarf.unit(header)?;
                        if let Some(offset) = offset.to_unit_offset(&unit) {
                            if let Ok(ndie) = unit.entry(offset) {
                                return get_var_name(dwarf, &unit, &ndie);
                            }
                        }
                    }
                    return Ok(None);
                }
                val => {
                    error!("Unimplemented for {:?}", val);
                    return Err(anyhow!("Unimplemented for {:?}", val));
                }
            };
        }

        Ok(None)
    } else {
        Err(anyhow!("This die is not a variable"))
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
            Some(Exprloc(expr)) => Ok(VariableLocation::Expression(expr)),
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
                    Ok(VariableLocation::LocationOutOfRange)
                } else {
                    Ok(VariableLocation::NoLocation)
                }
            }
            None => Ok(VariableLocation::NoLocation),
            Some(v) => {
                error!("Unimplemented for {:?}", v);
                Err(anyhow!("Unimplemented for {:?}", v))
            }
        }
    } else {
        Err(anyhow!("This die is not a variable"))
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
            Some(result) => Ok(result),
            None => {
                if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
                    match die_offset {
                        UnitRef(offset) => {
                            if let Ok(ao_die) = unit.entry(offset) {
                                return find_variable_type_die(dwarf, unit, &ao_die);
                            }
                        }
                        DebugInfoRef(di_offset) => {
                            let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                            let mut iter = dwarf.debug_info.units();
                            while let Ok(Some(header)) = iter.next() {
                                let unit = dwarf.unit(header)?;
                                if let Some(offset) = offset.to_unit_offset(&unit) {
                                    if let Ok(ndie) = unit.entry(offset) {
                                        return find_variable_type_die(dwarf, &unit, &ndie);
                                    }
                                }
                            }
                            return Err(anyhow!("Could not find this variables type die"));
                        }
                        val => {
                            error!("Unimplemented for {:?}", val);
                            return Err(anyhow!("Unimplemented for {:?}", val));
                        }
                    };
                }

                Err(anyhow!("Could not find this variables type die"))
            }
        }
    } else {
        Err(anyhow!("This die is not a variable"))
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
                    find_variable_source_information(dwarf, unit, &ao_die, cwd)
                }
                DebugInfoRef(di_offset) => {
                    let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                    let mut iter = dwarf.debug_info.units();
                    while let Ok(Some(header)) = iter.next() {
                        let unit = dwarf.unit(header)?;
                        if let Some(offset) = offset.to_unit_offset(&unit) {
                            if let Ok(ndie) = unit.entry(offset) {
                                return find_variable_source_information(dwarf, &unit, &ndie, cwd);
                            }
                        }
                    }
                    Err(anyhow!("Could not find this variables die"))
                }
                val => {
                    error!("Unimplemented for {:?}", val);
                    Err(anyhow!("Unimplemented for {:?}", val))
                }
            }
        } else {
            SourceInformation::get_die_source_information(dwarf, unit, die, cwd)
        }
    } else {
        Err(anyhow!("This die is not a variable"))
    }
}
