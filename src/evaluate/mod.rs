pub mod attributes;
pub mod evaluate;
pub mod pieces;
pub mod value_information;

use crate::call_stack::MemoryAccess;
use crate::evaluate::pieces::evaluate_pieces;
use crate::registers::Registers;

use gimli::{AttributeValue::UnitRef, DebuggingInformationEntry, Dwarf, Expression, Reader, Unit};

pub use evaluate::{
    convert_to_gimli_value, ArrayValue, BaseValue, EnumValue, EvaluatorValue, MemberValue,
    StructValue, UnionValue,
};

use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub enum EvalResult {
    Complete,
    RequiresRegister { register: u16 },
    RequiresMemory { address: u32, num_words: usize },
}

pub fn call_evaluate<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    nunit: &Unit<R>,
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
                    nunit,
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
                    let unit = dwarf.unit(header)?;
                    if let Some(offset) = offset.to_unit_offset(&unit) {
                        let die = unit.entry(offset)?;
                        return evaluate(
                            dwarf,
                            nunit,
                            pc,
                            expr,
                            frame_base,
                            Some(&unit),
                            Some(&die),
                            registers,
                            mem,
                        );
                    }
                }
                bail!("");
            }
            _ => bail!(""),
        };
    } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
        match die_offset {
            UnitRef(offset) => {
                if let Ok(ndie) = unit.entry(offset) {
                    return call_evaluate(
                        dwarf, nunit, pc, expr, frame_base, unit, &ndie, registers, mem,
                    );
                }
            }
            _ => {
                unimplemented!();
            }
        };
    }
    bail!("");
}

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
    return EvaluatorValue::evaluate_variable(registers, mem, &pieces);

    //let mut evaluator = evaluate::Evaluator::new(pieces.clone(), type_unit, type_die);
    //let value = evaluator.evaluate(&dwarf, registers, mem)?;

    //Ok(value)
}
