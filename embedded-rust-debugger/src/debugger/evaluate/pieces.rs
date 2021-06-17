use std::collections::HashMap;

use super::{
    call_evaluate,
    EvalResult,
    EvaluatorResult,
    evaluate,
};

use gimli::{
    Dwarf,
    Unit,
    EvaluationResult::{
        Complete,
        RequiresMemory,
        RequiresRegister,
        RequiresFrameBase,
        RequiresTls,
        RequiresCallFrameCfa,
        RequiresAtLocation,
        RequiresEntryValue,
        RequiresParameterRef,
        RequiresRelocatedAddress,
        RequiresIndexedAddress,
        RequiresBaseType,
    },
    Reader,
    Evaluation,
    EvaluationResult,
    UnitOffset,
    Register,
    Expression,
    DieReference,
};


pub use super::value::{
    EvaluatorValue,
    StructValue,
    EnumValue,
    MemberValue,
    UnionValue,
    ArrayValue,
    convert_to_gimli_value,
    BaseValue,
};


use super::evaluate::{
    parse_base_type,
};


use anyhow::{
    Result,
    anyhow,
};


pub enum EvalPieceResult<R: Reader<Offset = usize>> {
    Complete(Vec<gimli::Piece<R>>),
    Requires(EvalResult),
}


pub fn evaluate_pieces<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                unit:       &Unit<R>,
                pc:         u32,
                expr:       Expression<R>,
                frame_base: Option<u64>,
                type_unit:  Option<&gimli::Unit<R>>,
                registers:  &HashMap<u16, u32>,
                memory:     &HashMap<u32, u32>,
                ) -> Result<EvalPieceResult<R>>
{
    let mut eval    = expr.evaluation(unit.encoding());
    let mut result  = eval.evaluate()?;

    println!("fb: {:?}, pc: {:?}", frame_base, pc);
    loop {
        //println!("{:#?}", result);
        let resolved = match result {
            Complete => break,
            RequiresMemory{address, size, space, base_type} =>
                resolve_requires_mem(unit,
                                     &mut eval,
                                     &mut result,
                                     address,
                                     size,
                                     space,
                                     base_type,
                                     memory)?,

            RequiresRegister{register, base_type} =>
                resolve_requires_reg(unit,
                                     &mut eval,
                                     &mut result,
                                     register,
                                     base_type,
                                     registers)?,

            RequiresFrameBase => {
                result = eval.resume_with_frame_base(frame_base.unwrap())?;
                EvalResult::Complete
            }, // TODO: Check and test if correct.

            RequiresTls(_tls) =>
                unimplemented!(), // TODO

            RequiresCallFrameCfa =>
                unimplemented!(), // TODO

            RequiresAtLocation(die_ref) =>
                resolve_requires_at_location(dwarf,
                                             unit,
                                             pc,
                                             &mut eval,
                                             &mut result,
                                             frame_base,
                                             die_ref,
                                             registers,
                                             memory)?,

            RequiresEntryValue(entry) => unimplemented!(),//resolve_requires_entry_value(dwarf, // TODO
//                                                                  unit,
//                                                                  &mut eval,
//                                                                  &mut result,
//                                                                  entry.clone(),
//                                                                  pc,
//                                                                  frame_base,
//                                                                  registers,
//                                                                  memory)?,

            RequiresParameterRef(unit_offset) => resolve_requires_paramter_ref(dwarf, unit, &mut eval, &mut result, unit_offset, type_unit, pc, frame_base, registers, memory)?,

            RequiresRelocatedAddress(num) => resolve_requires_relocated_address(&mut eval, &mut result, num)?,

            RequiresIndexedAddress {index, relocate: _} => resolve_requires_indexed_address(dwarf, unit, &mut eval, &mut result, index)?,

            RequiresBaseType(unit_offset) => resolve_requires_base_type(unit, &mut eval, &mut result, unit_offset)?,
        };

        match resolved {
            EvalResult::Complete => continue,
            _ => return Ok(EvalPieceResult::Requires(resolved)),
        };
    }

    let pieces = eval.result();
    println!("{:#?}", pieces);
    Ok(EvalPieceResult::Complete(pieces))
}


fn resolve_requires_at_location<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                unit:       &Unit<R>,
                                pc:         u32,
                                eval:       &mut Evaluation<R>,
                                result:     &mut EvaluationResult<R>,
                                frame_base: Option<u64>,
                                die_ref:    DieReference<usize>,
                                registers:  &HashMap<u16, u32>,
                                memory:  &HashMap<u32, u32>,
                                ) -> Result<EvalResult>
                                where R: Reader<Offset = usize>
{ 
    match die_ref {
        DieReference::UnitRef(unit_offset) => {
            return help_at_location(dwarf, unit, pc, eval, result, frame_base, unit_offset, registers, memory);
        },

        DieReference::DebugInfoRef(debug_info_offset) => {
            let unit_header = dwarf.debug_info.header_from_offset(debug_info_offset)?;
            if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                let new_unit = dwarf.unit(unit_header)?;
                return help_at_location(dwarf, &new_unit, pc, eval, result, frame_base, unit_offset, registers, memory);
            } else {
                return Err(anyhow!("Could not find at location"));
            }    
        },
    };
}


/*
 * Resolves requires memory when evaluating a die.
 * TODO: Check and test if correct.
 */
fn resolve_requires_mem<R: Reader<Offset = usize>>(unit:       &Unit<R>,
                                                   eval:       &mut Evaluation<R>,
                                                   result:     &mut EvaluationResult<R>,
                                                   address:    u64,
                                                   size:       u8, // TODO: Handle size
                                                   space:      Option<u64>, // TODO: Handle space
                                                   base_type:  UnitOffset<usize>,
                                                   memory:      &HashMap<u32, u32>,
                                                   ) -> Result<EvalResult>
                                                   where R: Reader<Offset = usize>
{
    println!("address: {:?}, size: {:?}, space: {:?}", address, size, space);
    match memory.get(&(address as u32)) { //TODO handle size and space
        Some(data) => {
            let value = parse_base_type(unit, &[*data], base_type);
            *result = eval.resume_with_memory(convert_to_gimli_value(value))?;    
            Ok(EvalResult::Complete)
        },
        None => Ok(EvalResult::RequiresMemory {
            address: address as u32,
            num_words:  (size as usize + 4 - 1)/4, 
        }),
    }
}


/*
 * Resolves requires register when evaluating a die.
 * TODO: Check and test if correct.
 */
fn resolve_requires_reg<R: Reader<Offset = usize>>(
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        reg:        Register,
                        base_type:  UnitOffset<usize>,
                        registers:  &HashMap<u16, u32>,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    println!("req reg: {:?}", reg.0);
    match registers.get(&reg.0) {
        Some(data) => {
            let value   = parse_base_type(unit, &[*data], base_type);
            *result     = eval.resume_with_register(convert_to_gimli_value(value))?;

            Ok(EvalResult::Complete)
        },
        None => Ok(EvalResult::RequiresRegister {
            register: reg.0,
        }),
    }
}


fn resolve_requires_entry_value<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        entry:      gimli::Expression<R>,
                        pc: u32,
                        frame_base: Option<u64>,
                        registers:  &HashMap<u16, u32>,
                        memory:     &HashMap<u32, u32>,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    let entry_value = match evaluate(dwarf, unit, pc, entry, frame_base, None, None, registers, memory)? {
        EvaluatorResult::Complete(val) => val,
        EvaluatorResult::Requires(req) => return Ok(req),
    };

    *result = eval.resume_with_entry_value(convert_to_gimli_value(entry_value.to_value().unwrap()))?;

    Ok(EvalResult::Complete)
}


fn resolve_requires_paramter_ref<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        unit_offset: UnitOffset,
                        type_unit:  Option<&gimli::Unit<R>>,
                        pc: u32,
                        frame_base: Option<u64>,
                        registers:  &HashMap<u16, u32>,
                        memory:     &HashMap<u32, u32>,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    let die     = unit.entry(unit_offset)?;
    let expr    = die.attr_value(gimli::DW_AT_call_value)?.unwrap().exprloc_value().unwrap();
    let value   = match evaluate(dwarf, unit, pc, expr, frame_base, type_unit, Some(&die), registers, memory)? {
        EvaluatorResult::Complete(val) => val,
        EvaluatorResult::Requires(req) => return Ok(req),
    };

    if let EvaluatorValue::Value(BaseValue::U64(val)) = value {
        *result = eval.resume_with_parameter_ref(val)?;
    } else {
        panic!("here");
        //return Err(anyhow!("could not find parameter"));
    }

    Ok(EvalResult::Complete)
}


fn resolve_requires_relocated_address<R: Reader<Offset = usize>>(
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        num: u64
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    *result = eval.resume_with_relocated_address(num)?; // TODO: Check and test if correct.

    Ok(EvalResult::Complete)
}


fn resolve_requires_indexed_address<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        index:      gimli::DebugAddrIndex,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    // TODO: Check and test if correct. Also handle relocate flag
    *result = eval.resume_with_indexed_address(dwarf.address(unit, index)?)?;

    Ok(EvalResult::Complete)
}


fn resolve_requires_base_type<R: Reader<Offset = usize>>(
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        unit_offset: UnitOffset,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    // TODO: Check and test if correct

    *result = eval.resume_with_base_type(convert_to_gimli_value(parse_base_type(unit, &[0], unit_offset)).value_type())?;

    Ok(EvalResult::Complete)
}



fn help_at_location<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                    unit:           &Unit<R>,
                    pc:             u32,
                    eval:           &mut Evaluation<R>,
                    result:         &mut EvaluationResult<R>,
                    frame_base:     Option<u64>,
                    unit_offset:    UnitOffset<usize>,
                    registers:  &HashMap<u16, u32>,
                    memory:  &HashMap<u32, u32>,
                    ) -> Result<EvalResult>
                    where R: Reader<Offset = usize>
{
    let die = unit.entry(unit_offset)?;
    if let Some(expr) = die.attr_value(gimli::DW_AT_location)?.unwrap().exprloc_value() {
       
        let val = match call_evaluate(dwarf, &unit, pc, expr, frame_base, &unit, &die, registers, memory)? {
            EvaluatorResult::Complete(val) => val,
            EvaluatorResult::Requires(req) => return Ok(req),
        };

        if let EvaluatorValue::Bytes(b) = val {
           *result =  eval.resume_with_at_location(b)?;
           return Ok(EvalResult::Complete);
        } else {
            return Err(anyhow!("Error expected bytes"));
        }
    }
    else {
        return Err(anyhow!("die has no at location"));
    }
}

