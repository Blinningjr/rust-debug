pub mod value;
pub mod evaluate;
pub mod attributes;
pub mod pieces;
pub mod value_information;

use crate::call_stack::MemoryAccess;
use pieces::EvalPieceResult;
use crate::evaluate::pieces::evaluate_pieces;
use crate::memory_and_registers::MemoryAndRegisters;


use gimli::{
    Dwarf,
    Unit,
    Expression,
    Reader,
    DebuggingInformationEntry,
    AttributeValue::{
        UnitRef,
    },
};

pub use value::{
    EvaluatorValue,
    StructValue,
    EnumValue,
    MemberValue,
    UnionValue,
    ArrayValue,
    convert_to_gimli_value,
    BaseValue,
};

use anyhow::{
    bail,
    Result,
};


#[derive(Debug, Clone)]
pub enum EvalResult {
    Complete,
    RequiresRegister { register: u16 },
    RequiresMemory { address: u32, num_words: usize },
}


#[derive(Debug, Clone)]
pub enum EvaluatorResult<R: Reader<Offset = usize>> {
    Complete(EvaluatorValue<R>),
    Requires(EvalResult),
}


pub fn call_evaluate<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: & Dwarf<R>,
                                                nunit:      &Unit<R>,
                                                pc:         u32,
                                                expr:       gimli::Expression<R>,
                                                frame_base: Option<u64>,
                                                unit:     &Unit<R>,
                                                die: &DebuggingInformationEntry<R>,
                                                memory_and_registers: &MemoryAndRegisters,
                                                mem:                         &mut T,
                                                ) -> Result<EvaluatorResult<R>>
{
    if let Ok(Some(tattr)) =  die.attr_value(gimli::DW_AT_type) {
        match tattr {
            gimli::AttributeValue::UnitRef(offset) => {
                let die = unit.entry(offset)?;
                return evaluate(dwarf, nunit, pc, expr, frame_base, Some(unit), Some(&die), memory_and_registers, mem);
            },
            gimli::AttributeValue::DebugInfoRef(di_offset) => {
                let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                let mut iter = dwarf.debug_info.units();
                while let Ok(Some(header)) = iter.next() {
                    let unit = dwarf.unit(header)?;
                    if let Some(offset) = offset.to_unit_offset(&unit) {
                        let die = unit.entry(offset)?;
                        return evaluate(dwarf, nunit, pc, expr, frame_base, Some(&unit), Some(&die), memory_and_registers, mem);
                    }
                }
                bail!("");
            },
            _ => bail!(""),
        };
    } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
        match die_offset {
            UnitRef(offset) => {
                if let Ok(ndie) = unit.entry(offset) {
                    return call_evaluate(dwarf, nunit, pc, expr, frame_base, unit, &ndie, memory_and_registers, mem);
                }
            },
            _ => {
                unimplemented!();
            },
        };
    }
    bail!("");
}


pub fn evaluate<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: & Dwarf<R>,
                unit:       &Unit<R>,
                pc:         u32,
                expr:       Expression<R>,
                frame_base: Option<u64>,
                type_unit:  Option<&gimli::Unit<R>>,
                type_die:   Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
                memory_and_registers: &MemoryAndRegisters,
                mem:                         &mut T,
                ) -> Result<EvaluatorResult<R>>
{
    let pieces = match evaluate_pieces(dwarf, unit, pc, expr, frame_base, memory_and_registers, mem)? {
        EvalPieceResult::Complete(val) => val,
        EvalPieceResult::Requires(req) => return Ok(EvaluatorResult::Requires(req)),
    };
    evaluate_value(dwarf, pieces, type_unit, type_die, memory_and_registers, mem)
}


pub fn evaluate_value<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: &Dwarf<R>,
                                                 pieces:     Vec<gimli::Piece<R>>,
                                                 type_unit:  Option<&gimli::Unit<R>>,
                                                 type_die:   Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
                                                 memory_and_registers: &MemoryAndRegisters,
                                                mem:                         &mut T,
                                                 ) -> Result<EvaluatorResult<R>>
{
    let mut evaluator = evaluate::Evaluator::new(pieces.clone(), type_unit, type_die);
    loop {
        match evaluator.evaluate(&dwarf, memory_and_registers, mem)? {
            evaluate::EvaluatorResult::Complete => break,
            evaluate::EvaluatorResult::RequireReg(reg) => { 
                return Ok(EvaluatorResult::Requires(EvalResult::RequiresRegister {
                    register: reg,
                }));
            },
            evaluate::EvaluatorResult::RequireData {address, num_words} => {
                return Ok(EvaluatorResult::Requires(EvalResult::RequiresMemory {
                    address: address,
                    num_words: num_words,
                }));
            },
        };
    }

    let value = match evaluator.get_value() {
        Some(val) => val,
        None => unreachable!(),
    };

    Ok(EvaluatorResult::Complete(value))
}

