use crate::call_stack::MemoryAccess;
use crate::registers::Registers;

use super::{call_evaluate, evaluate, EvalResult, EvaluatorResult};

use gimli::{
    DieReference, Dwarf, Evaluation, EvaluationResult,
    EvaluationResult::{
        Complete, RequiresAtLocation, RequiresBaseType, RequiresCallFrameCfa, RequiresEntryValue,
        RequiresFrameBase, RequiresIndexedAddress, RequiresMemory, RequiresParameterRef,
        RequiresRegister, RequiresRelocatedAddress, RequiresTls,
    },
    Expression, Reader, Unit, UnitOffset,
};

pub use super::value::{
    convert_to_gimli_value, ArrayValue, BaseValue, EnumValue, EvaluatorValue, MemberValue,
    StructValue, UnionValue,
};

use super::evaluate::parse_base_type;

use anyhow::{anyhow, bail, Result};

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
                    let value = parse_base_type(unit, data, base_type)?;
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
                    let value = parse_base_type(unit, bytes, base_type)?;
                    result = eval.resume_with_register(convert_to_gimli_value(value))?;
                }
                None => {
                    return Err(anyhow!("Requires register {}", register.0));
                }
            },

            RequiresFrameBase => {
                result = eval.resume_with_frame_base(match frame_base {
                    Some(val) => val,
                    None => bail!("Requires frame base"), // TODO: Return Error instead
                })?;
            }

            RequiresTls(_tls) => unimplemented!(), // TODO

            RequiresCallFrameCfa => unimplemented!(), // TODO: Add CFA to Register struct

            RequiresAtLocation(die_ref) => match die_ref {
                DieReference::UnitRef(unit_offset) => match help_at_location(
                    dwarf,
                    unit,
                    pc,
                    &mut eval,
                    &mut result,
                    frame_base,
                    unit_offset,
                    registers,
                    mem,
                )? {
                    EvalResult::RequiresMemory {
                        address: _,
                        num_words: _,
                    } => bail!("requires mem"),
                    EvalResult::RequiresRegister { register: _ } => bail!("requires regs"),
                    EvalResult::Complete => (),
                },

                DieReference::DebugInfoRef(debug_info_offset) => {
                    let unit_header = dwarf.debug_info.header_from_offset(debug_info_offset)?;
                    if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                        let new_unit = dwarf.unit(unit_header)?;
                        match help_at_location(
                            dwarf,
                            &new_unit,
                            pc,
                            &mut eval,
                            &mut result,
                            frame_base,
                            unit_offset,
                            registers,
                            mem,
                        )? {
                            EvalResult::RequiresMemory {
                                address: _,
                                num_words: _,
                            } => {
                                bail!("requires mem")
                            }
                            EvalResult::RequiresRegister { register: _ } => bail!("requires regs"),
                            EvalResult::Complete => (),
                        }
                    } else {
                        return Err(anyhow!("Could not find at location"));
                    }
                }
            },

            RequiresEntryValue(entry) => {
                let entry_value = match evaluate(
                    dwarf, unit, pc, entry, frame_base, None, None, registers, mem,
                )? {
                    EvaluatorResult::Complete(val) => val,
                    EvaluatorResult::Requires(_req) => {
                        return Err(anyhow!("Requires Memory or register"))
                    }
                };

                result = eval.resume_with_entry_value(convert_to_gimli_value(match entry_value
                    .to_value()
                {
                    Some(val) => val,
                    None => bail!("Optimised Out"),
                }))?;
            }

            RequiresParameterRef(unit_offset) => {
                let die = unit.entry(unit_offset)?;
                let call_value = match die.attr_value(gimli::DW_AT_call_value)? {
                    Some(val) => val,
                    None => bail!("Could not find required paramter"),
                };

                let expr = match call_value.exprloc_value() {
                    Some(val) => val,
                    None => bail!("Could not find required paramter"),
                };
                let value = match evaluate(
                    dwarf,
                    unit,
                    pc,
                    expr,
                    frame_base,
                    Some(unit),
                    Some(&die),
                    registers,
                    mem,
                )? {
                    EvaluatorResult::Complete(val) => val,
                    EvaluatorResult::Requires(_req) => return Err(anyhow!("Requires mem or reg")),
                };

                if let EvaluatorValue::Value(BaseValue::U64(val), _) = value {
                    result = eval.resume_with_parameter_ref(val)?;
                } else {
                    bail!("Could not find required paramter");
                }
            }

            RequiresRelocatedAddress(_num) => {
                unimplemented!();
                //                result = eval.resume_with_relocated_address(num)?; // TODO: Check and test if correct.
            }

            RequiresIndexedAddress {
                index: _,
                relocate: _,
            } => {
                // TODO: Check and test if correct. Also handle relocate flag
                unimplemented!();
                //                result = eval.resume_with_indexed_address(dwarf.address(unit, index)?)?;
            }

            RequiresBaseType(unit_offset) => {
                let die = unit.entry(unit_offset)?;
                let mut attrs = die.attrs();
                while let Some(attr) = attrs.next().unwrap() {
                    println!("Attribute name = {:?}", attr.name());
                    println!("Attribute value = {:?}", attr.value());
                }
                unimplemented!();
            }
        };
    }

    Ok(eval.result())
}

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
) -> Result<EvalResult>
where
    R: Reader<Offset = usize>,
{
    let die = unit.entry(unit_offset)?;
    let location = match die.attr_value(gimli::DW_AT_location)? {
        Some(val) => val,
        None => bail!("Could not find location attribute"),
    };
    if let Some(expr) = location.exprloc_value() {
        let val = match call_evaluate(
            dwarf, &unit, pc, expr, frame_base, &unit, &die, registers, mem,
        )? {
            EvaluatorResult::Complete(val) => val,
            EvaluatorResult::Requires(req) => return Ok(req),
        };

        if let EvaluatorValue::Bytes(b) = val {
            *result = eval.resume_with_at_location(b)?;
            return Ok(EvalResult::Complete);
        } else {
            bail!("Error expected bytes");
        }
    } else {
        bail!("die has no at location");
    }
}
