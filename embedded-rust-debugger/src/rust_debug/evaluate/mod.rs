pub mod value;
pub mod evaluate;
pub mod attributes;
pub mod pieces;


use pieces::EvalPieceResult;
use crate::rust_debug::evaluate::pieces::evaluate_pieces;
use crate::rust_debug::memory_and_registers::MemoryAndRegisters;

use super::{
    call_evaluate,
};

use gimli::{
    Dwarf,
    Unit,
    Expression,
    Reader,
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


pub fn evaluate<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                unit:       &Unit<R>,
                pc:         u32,
                expr:       Expression<R>,
                frame_base: Option<u64>,
                type_unit:  Option<&gimli::Unit<R>>,
                type_die:   Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
                memory_and_registers: &MemoryAndRegisters,
                ) -> Result<EvaluatorResult<R>>
{
    let pieces = match evaluate_pieces(dwarf, unit, pc, expr, frame_base, memory_and_registers)? {
        EvalPieceResult::Complete(val) => val,
        EvalPieceResult::Requires(req) => return Ok(EvaluatorResult::Requires(req)),
    };
    evaluate_value(dwarf, pieces, type_unit, type_die, memory_and_registers)
}


pub fn evaluate_value<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                                                 pieces:     Vec<gimli::Piece<R>>,
                                                 type_unit:  Option<&gimli::Unit<R>>,
                                                 type_die:   Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
                                                 memory_and_registers: &MemoryAndRegisters,
                                                 ) -> Result<EvaluatorResult<R>>
{
    let mut evaluator = evaluate::Evaluator::new(&dwarf, pieces.clone(), type_unit, type_die);
    loop {
        match evaluator.evaluate(&dwarf, memory_and_registers) {
            evaluate::EvaluatorResult::Complete => break,
            evaluate::EvaluatorResult::RequireReg(reg) => { 
                println!("read reg: {:?}", reg);
                return Ok(EvaluatorResult::Requires(EvalResult::RequiresRegister {
                    register: reg,
                }));
            },
            evaluate::EvaluatorResult::RequireData {address, num_words} => {
                println!("address: {:?}, num_words: {:?}", address, num_words);
                return Ok(EvaluatorResult::Requires(EvalResult::RequiresMemory {
                    address: address,
                    num_words: num_words,
                }));
            },
        };
    }

    let value = evaluator.get_value();

//      println!("Value: {:#?}", value);
    Ok(EvaluatorResult::Complete(value.unwrap()))
}

