/// Contains functions for retrieving the values of some of the DWARF attributes.
pub mod attributes;

/// Contains structs representing the different Rust data types and more.
pub mod evaluate;

use crate::call_stack::MemoryAccess;
use crate::registers::Registers;
use anyhow::{anyhow, Result};
use evaluate::{convert_to_gimli_value, BaseTypeValue, EvaluatorValue};
use gimli::{
    AttributeValue::UnitRef,
    DebuggingInformationEntry, DieReference, Dwarf, Evaluation, EvaluationResult,
    EvaluationResult::{
        Complete, RequiresAtLocation, RequiresBaseType, RequiresCallFrameCfa, RequiresEntryValue,
        RequiresFrameBase, RequiresIndexedAddress, RequiresMemory, RequiresParameterRef,
        RequiresRegister, RequiresRelocatedAddress, RequiresTls,
    },
    Expression, Reader, Unit, UnitOffset,
};
use log::{debug, error};
use std::convert::TryInto;

/// Will find the DIE representing the type can evaluate the variable.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `pc` - A machine code address, usually the current code location.
/// * `expr` - The expression to be evaluated.
/// * `frame_base` - The frame base address value.
/// * `unit` - A compilation unit which contains the given DIE.
/// * `die` - The DIE the is used to find the DIE representing the type.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// This function is used to find the DIE representing the type and then to evaluate the value of
/// the given DIE>
pub fn call_evaluate<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    pc: u32,
    expr: gimli::Expression<R>,
    frame_base: Option<u64>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
    registers: &Registers,
    mem: &mut T,
) -> Result<EvaluatorValue<R>> {
    if let Ok(Some(tattr)) = die.attr_value(gimli::DW_AT_type) {
        match tattr {
            gimli::AttributeValue::UnitRef(offset) => {
                let die = unit.entry(offset)?;
                return evaluate(
                    dwarf,
                    unit,
                    pc,
                    expr,
                    frame_base,
                    Some(unit),
                    Some(&die),
                    registers,
                    mem,
                );
            }
            gimli::AttributeValue::DebugInfoRef(di_offset) => {
                let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                let mut iter = dwarf.debug_info.units();
                while let Ok(Some(header)) = iter.next() {
                    let type_unit = dwarf.unit(header)?;
                    if let Some(offset) = offset.to_unit_offset(&type_unit) {
                        let die = type_unit.entry(offset)?;
                        return evaluate(
                            dwarf,
                            unit,
                            pc,
                            expr,
                            frame_base,
                            Some(&type_unit),
                            Some(&die),
                            registers,
                            mem,
                        );
                    }
                }

                error!("Unreachable");
                return Err(anyhow!("Unreachable"));
            }
            attribute => {
                error!("Unimplemented for attribute {:?}", attribute);
                return Err(anyhow!("Unimplemented for attribute {:?}", attribute));
            }
        };
    } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
        match die_offset {
            UnitRef(offset) => {
                if let Ok(ndie) = unit.entry(offset) {
                    return call_evaluate(dwarf, pc, expr, frame_base, unit, &ndie, registers, mem);
                }
            }
            _ => {
                error!("Unimplemented");
                return Err(anyhow!("Unimplemented"));
            }
        };
    }

    error!("Unreachable");
    return Err(anyhow!("Unreachable"));
}

/// Will evaluate the value of the given DWARF expression.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A compilation unit which contains the given DIE.
/// * `pc` - A machine code address, usually the current code location.
/// * `expr` - The expression to be evaluated.
/// * `frame_base` - The frame base address value.
/// * `type_unit` - A compilation unit which contains the given DIE which represents the type of
/// the given expression. None if the expression does not have a type.
/// * `type_die` - The DIE the represents the type of the given expression. None if the expression
/// does not have a type.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// This function will first evaluate the expression into gimli-rs `Piece`s.
/// Then it will use the pieces and the type too evaluate and parse the value.
pub fn evaluate<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    pc: u32,
    expr: Expression<R>,
    frame_base: Option<u64>,
    type_unit: Option<&gimli::Unit<R>>,
    type_die: Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
    registers: &Registers,
    mem: &mut T,
) -> Result<EvaluatorValue<R>> {
    let pieces = evaluate_pieces(dwarf, unit, pc, expr, frame_base, registers, mem)?;
    evaluate_value(dwarf, pieces, type_unit, type_die, registers, mem)
}

/// Will evaluate the value of the given list of gimli-rs `Piece`s.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `pieces` - A list of gimli-rs pieces containing the location information..
/// * `type_unit` - A compilation unit which contains the given DIE which represents the type of
/// the given expression. None if the expression does not have a type.
/// * `type_die` - The DIE the represents the type of the given expression. None if the expression
/// does not have a type.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// Then it will use the pieces and the type too evaluate and parse the value.
pub fn evaluate_value<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    pieces: Vec<gimli::Piece<R>>,
    type_unit: Option<&gimli::Unit<R>>,
    type_die: Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
    registers: &Registers,
    mem: &mut T,
) -> Result<EvaluatorValue<R>> {
    match type_unit {
        Some(unit) => match type_die {
            Some(die) => {
                debug!("with type info");
                return EvaluatorValue::evaluate_variable_with_type(
                    dwarf,
                    registers,
                    mem,
                    &pieces,
                    unit.header.offset(),
                    die.offset(),
                );
            }
            None => (),
        },
        None => (),
    };
    debug!("without type info");
    return EvaluatorValue::evaluate_variable(registers, mem, &pieces);
}

/// Evaluates a gimli-rs `Expression` into a `Vec` of `Piece`s.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A compilation unit which contains the given DIE.
/// * `pc` - A machine code address, usually the current code location.
/// * `expr` - The expression to be evaluated into `Piece`s.
/// * `frame_base` - The frame base address value.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// This function will evaluate the given expression into a list of pieces.
/// These pieces describe the size and location of the variable the given expression is from.
pub fn evaluate_pieces<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    pc: u32,
    expr: Expression<R>,
    frame_base: Option<u64>,
    registers: &Registers,
    mem: &mut T,
) -> Result<Vec<gimli::Piece<R>>> {
    let mut eval = expr.evaluation(unit.encoding());
    let mut result = eval.evaluate()?;

    loop {
        match result {
            Complete => break,
            RequiresMemory {
                address,
                size,
                space: _, // Do not know what this is used for.
                base_type,
            } => match mem.get_address(&(address as u32), size as usize) {
                Some(data) => {
                    let value = eval_base_type(unit, data, base_type)?;
                    result = eval.resume_with_memory(convert_to_gimli_value(value))?;
                }
                None => {
                    return Err(anyhow!("Requires Memory"));
                }
            },

            RequiresRegister {
                register,
                base_type,
            } => match registers.get_register_value(&register.0) {
                Some(data) => {
                    let bytes = data.to_le_bytes().to_vec();
                    let value = eval_base_type(unit, bytes, base_type)?;
                    result = eval.resume_with_register(convert_to_gimli_value(value))?;
                }
                None => {
                    return Err(anyhow!("Requires register {}", register.0));
                }
            },

            RequiresFrameBase => {
                result = eval.resume_with_frame_base(match frame_base {
                    Some(val) => val,
                    None => {
                        error!("Requires frame base");
                        return Err(anyhow!("Requires frame base"));
                    }
                })?;
            }

            RequiresTls(_tls) => {
                error!("Unimplemented");
                return Err(anyhow!("Unimplemented")); // TODO
            }

            RequiresCallFrameCfa => {
                result = eval.resume_with_call_frame_cfa(
                    registers.cfa.ok_or(anyhow!("Requires CFA"))? as u64,
                )?;
            }

            RequiresAtLocation(die_ref) => match die_ref {
                DieReference::UnitRef(unit_offset) => help_at_location(
                    dwarf,
                    unit,
                    pc,
                    &mut eval,
                    &mut result,
                    frame_base,
                    unit_offset,
                    registers,
                    mem,
                )?,

                DieReference::DebugInfoRef(debug_info_offset) => {
                    let unit_header = dwarf.debug_info.header_from_offset(debug_info_offset)?;
                    if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                        let new_unit = dwarf.unit(unit_header)?;
                        help_at_location(
                            dwarf,
                            &new_unit,
                            pc,
                            &mut eval,
                            &mut result,
                            frame_base,
                            unit_offset,
                            registers,
                            mem,
                        )?;
                    } else {
                        return Err(anyhow!("Could not find at location"));
                    }
                }
            },

            RequiresEntryValue(entry) => {
                let entry_value = evaluate(
                    dwarf, unit, pc, entry, frame_base, None, None, registers, mem,
                )?;

                result = eval.resume_with_entry_value(convert_to_gimli_value(match entry_value
                    .to_value()
                {
                    Some(val) => val,
                    None => {
                        error!("Optimized Out");
                        return Err(anyhow!("Optimized Out"));
                    }
                }))?;
            }

            RequiresParameterRef(unit_offset) => {
                let die = unit.entry(unit_offset)?;
                let call_value = match die.attr_value(gimli::DW_AT_call_value)? {
                    Some(val) => val,
                    None => {
                        error!("Could not find required paramter");
                        return Err(anyhow!("Could not find required parameter"));
                    }
                };

                let expr = match call_value.exprloc_value() {
                    Some(val) => val,
                    None => {
                        error!("Could not find required paramter");
                        return Err(anyhow!("Could not find required parameter"));
                    }
                };
                let value = evaluate(
                    dwarf,
                    unit,
                    pc,
                    expr,
                    frame_base,
                    Some(unit),
                    Some(&die),
                    registers,
                    mem,
                )?;

                if let EvaluatorValue::Value(BaseTypeValue::U64(val), _) = value {
                    result = eval.resume_with_parameter_ref(val)?;
                } else {
                    error!("Could not find required paramter");
                    return Err(anyhow!("Could not find required parameter"));
                }
            }

            RequiresRelocatedAddress(_num) => {
                error!("Unimplemented");
                return Err(anyhow!("Unimplemented"));
                //                result = eval.resume_with_relocated_address(num)?; // TODO: Check and test if correct.
            }

            RequiresIndexedAddress {
                index: _,
                relocate: _,
            } => {
                // TODO: Check and test if correct. Also handle relocate flag
                error!("Unimplemented");
                return Err(anyhow!("Unimplemented"));
                //                result = eval.resume_with_indexed_address(dwarf.address(unit, index)?)?;
            }

            RequiresBaseType(unit_offset) => {
                let die = unit.entry(unit_offset)?;
                let mut attrs = die.attrs();
                while let Some(attr) = match attrs.next() {
                    Ok(val) => val,
                    Err(err) => {
                        error!("{:?}", err);
                        return Err(anyhow!("{:?}", err));
                    }
                } {
                    println!("Attribute name = {:?}", attr.name());
                    println!("Attribute value = {:?}", attr.value());
                }

                error!("Unimplemented");
                return Err(anyhow!("Unimplemented"));
            }
        };
    }

    Ok(eval.result())
}

/// Will parse the value of a `DW_TAG_base_type`.
///
/// Description:
///
/// * `unit` - A compilation unit which contains the type DIE pointed to by the given offset.
/// * `unit` - The value to parse in bytes.
/// * `base_type` - A offset into the given compilation unit which points to a DIE with the tag
/// `DW_TAG_base_type`.
///
/// This function will parse the given value into the type given by the offset `base_type`.
fn eval_base_type<R>(
    unit: &gimli::Unit<R>,
    data: Vec<u8>,
    base_type: gimli::UnitOffset<usize>,
) -> Result<BaseTypeValue>
where
    R: Reader<Offset = usize>,
{
    if base_type.0 == 0 {
        // NOTE: length can't be more then one word
        let value = match data.len() {
            0 => 0,
            1 => u8::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            }) as u64,
            2 => u16::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            }) as u64,
            4 => u32::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            }) as u64,
            8 => u64::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            }),
            _ => {
                error!("Unreachable");
                return Err(anyhow!("Unreachable"));
            }
        };
        return Ok(BaseTypeValue::Generic(value));
    }
    let die = unit.entry(base_type)?;

    // I think that the die returned must be a base type tag.
    if die.tag() != gimli::DW_TAG_base_type {
        error!("Requires at the die has tag DW_TAG_base_type");
        return Err(anyhow!("Requires at the die has tag DW_TAG_base_type"));
    }

    let encoding = match die.attr_value(gimli::DW_AT_encoding)? {
        Some(gimli::AttributeValue::Encoding(dwate)) => dwate,
        _ => {
            error!("Expected base type die to have attribute DW_AT_encoding");
            return Err(anyhow!(
                "Expected base type die to have attribute DW_AT_encoding"
            ));
        }
    };

    BaseTypeValue::parse_base_type(data, encoding)
}

/// Will evaluate a value that is required when evaluating a expression into pieces.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A compilation unit which contains the given DIE.
/// * `pc` - A machine code address, usually the current code location.
/// * `eval` - A gimli-rs `Evaluation` that will be continued with the new value.
/// * `result` - A gimli-rs `EvaluationResult` that will be updated with the new evaluation result.
/// * `frame_base` - The frame base address value.
/// * `unit_offset` - A offset to the DIE that will be evaluated and added to the given `Evaluation` struct.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// This function is a helper function for continuing a `Piece` evaluation where another value
/// needs to be evaluated first.
fn help_at_location<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    pc: u32,
    eval: &mut Evaluation<R>,
    result: &mut EvaluationResult<R>,
    frame_base: Option<u64>,
    unit_offset: UnitOffset<usize>,
    registers: &Registers,
    mem: &mut T,
) -> Result<()>
where
    R: Reader<Offset = usize>,
{
    let die = unit.entry(unit_offset)?;
    let location = match die.attr_value(gimli::DW_AT_location)? {
        Some(val) => val,
        None => {
            error!("Could not find location attribute");
            return Err(anyhow!("Could not find location attribute"));
        }
    };
    if let Some(expr) = location.exprloc_value() {
        let val = call_evaluate(dwarf, pc, expr, frame_base, &unit, &die, registers, mem)?;

        if let EvaluatorValue::Bytes(b) = val {
            *result = eval.resume_with_at_location(b)?;
            return Ok(());
        } else {
            error!("Error expected bytes");
            return Err(anyhow!("Error expected bytes"));
        }
    } else {
        error!("die has no at location");
        return Err(anyhow!("die has no at location"));
    }
}
