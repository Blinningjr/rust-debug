use crate::registers::Registers;
use crate::call_stack::MemoryAccess;

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
    bail,
    anyhow,
};


pub enum EvalPieceResult<R: Reader<Offset = usize>> {
    Complete(Vec<gimli::Piece<R>>),
    Requires(EvalResult),
}


pub fn evaluate_pieces<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: & Dwarf<R>,
                unit:       &Unit<R>,
                pc:         u32,
                expr:       Expression<R>,
                frame_base: Option<u64>,
                registers: &Registers,
                mem:                         &mut T,
                ) -> Result<EvalPieceResult<R>>
{
    let mut eval    = expr.evaluation(unit.encoding());
    let mut result  = eval.evaluate()?;

    loop {
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
                                     mem)?,

            RequiresRegister{register, base_type} =>
                resolve_requires_reg(unit,
                                     &mut eval,
                                     &mut result,
                                     register,
                                     base_type,
                                     registers)?,

            RequiresFrameBase => {
                result = eval.resume_with_frame_base(match frame_base {
                    Some(val) => val,
                    None => bail!("Requires frame base"),
                })?;
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
                                             mem)?,

            RequiresEntryValue(ref entry) => {
                let e = entry.clone();
                resolve_requires_entry_value(dwarf, unit, &mut eval,&mut result, e, pc, frame_base, registers, mem)?
            },

            RequiresParameterRef(unit_offset) => resolve_requires_paramter_ref(dwarf, unit, &mut eval, &mut result, unit_offset, pc, frame_base, registers, mem)?,

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
    Ok(EvalPieceResult::Complete(pieces))
}


fn resolve_requires_at_location<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: & Dwarf<R>,
                                unit:       &Unit<R>,
                                pc:         u32,
                                eval:       &mut Evaluation<R>,
                                result:     &mut EvaluationResult<R>,
                                frame_base: Option<u64>,
                                die_ref:    DieReference<usize>,
                                registers: &Registers,
                                mem:                         &mut T,
                                ) -> Result<EvalResult>
                                where R: Reader<Offset = usize>
{ 
    match die_ref {
        DieReference::UnitRef(unit_offset) => {
            return help_at_location(dwarf, unit, pc, eval, result, frame_base, unit_offset, registers, mem);
        },

        DieReference::DebugInfoRef(debug_info_offset) => {
            let unit_header = dwarf.debug_info.header_from_offset(debug_info_offset)?;
            if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                let new_unit = dwarf.unit(unit_header)?;
                return help_at_location(dwarf, &new_unit, pc, eval, result, frame_base, unit_offset, registers, mem);
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
fn resolve_requires_mem<R: Reader<Offset = usize>, T: MemoryAccess>(unit:       &Unit<R>,
                                                   eval:       &mut Evaluation<R>,
                                                   result:     &mut EvaluationResult<R>,
                                                   address:    u64,
                                                   size:       u8, // number of bytes
                                                   _space:      Option<u64>, // TODO: Handle space
                                                   base_type:  UnitOffset<usize>,
                                                mem:                         &mut T,
                                                   ) -> Result<EvalResult>
                                                   where R: Reader<Offset = usize>
{
    match mem.get_address(&(address as u32), size as usize) {
        Some(data) => {
            let value = parse_base_type(unit, data, base_type)?;
            *result = eval.resume_with_memory(convert_to_gimli_value(value))?;    
            Ok(EvalResult::Complete)
        },
        None => Ok(EvalResult::RequiresMemory {
            address: address as u32,
            num_words:  size as usize,
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
                        registers: &Registers,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    match registers.get_register_value(&reg.0) {
        Some(data) => {
            let bytes = data.to_le_bytes().to_vec();
            let value   = parse_base_type(unit, bytes, base_type)?;
            *result     = eval.resume_with_register(convert_to_gimli_value(value))?;

            Ok(EvalResult::Complete)
        },
        None => Ok(EvalResult::RequiresRegister {
            register: reg.0,
        }),
    }
}


fn resolve_requires_entry_value<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: &Dwarf<R>,
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        entry:      gimli::Expression<R>,
                        pc: u32,
                        frame_base: Option<u64>,
                        registers: &Registers,
                        mem:                         &mut T,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    let entry_value = match evaluate(dwarf, unit, pc, entry, frame_base, None, None, registers, mem)? {
        EvaluatorResult::Complete(val) => val,
        EvaluatorResult::Requires(req) => return Ok(req),
    };

    *result = eval.resume_with_entry_value(
        convert_to_gimli_value(
            match entry_value.to_value() {
                Some(val) => val,
                None => bail!("Optimised Out"),
            }))?;

    Ok(EvalResult::Complete)
}


fn resolve_requires_paramter_ref<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: &Dwarf<R>,
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        unit_offset: UnitOffset,
                        pc: u32,
                        frame_base: Option<u64>,
                        registers: &Registers,
                mem:                         &mut T,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    let die     = unit.entry(unit_offset)?;
    let call_value = match die.attr_value(gimli::DW_AT_call_value)? {
        Some(val) => val,
        None => bail!("Could not find required paramter"),
    };

    let expr    = match call_value.exprloc_value() {
        Some(val) => val,
        None => bail!("Could not find required paramter"),
    };
    let value   = match evaluate(dwarf, unit, pc, expr, frame_base, Some(unit), Some(&die), registers, mem)? {
        EvaluatorResult::Complete(val) => val,
        EvaluatorResult::Requires(req) => return Ok(req),
    };

    if let EvaluatorValue::Value(BaseValue::U64(val), _) = value {
        *result = eval.resume_with_parameter_ref(val)?;
    } else {
        bail!("Could not find required paramter");
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
                        _eval:       &mut Evaluation<R>,
                        _result:     &mut EvaluationResult<R>,
                        unit_offset: UnitOffset,
                        ) -> Result<EvalResult>
                        where R: Reader<Offset = usize>
{
    let die = unit.entry(unit_offset)?;
    let mut attrs = die.attrs();
    while let Some(attr) = attrs.next().unwrap() {
        println!("Attribute name = {:?}", attr.name());
        println!("Attribute value = {:?}", attr.value());
    }
    unimplemented!();
}



fn help_at_location<R: Reader<Offset = usize>, T: MemoryAccess>(dwarf: & Dwarf<R>,
                    unit:           &Unit<R>,
                    pc:             u32,
                    eval:           &mut Evaluation<R>,
                    result:         &mut EvaluationResult<R>,
                    frame_base:     Option<u64>,
                    unit_offset:    UnitOffset<usize>,
                    registers: &Registers,
                mem:                         &mut T,
                    ) -> Result<EvalResult>
                    where R: Reader<Offset = usize>
{
    let die = unit.entry(unit_offset)?;
    let location = match die.attr_value(gimli::DW_AT_location)? {
        Some(val) => val,
        None => bail!("Could not find location attribute"),
    };
    if let Some(expr) = location.exprloc_value() {
       
        let val = match call_evaluate(dwarf, &unit, pc, expr, frame_base, &unit, &die, registers, mem)? {
            EvaluatorResult::Complete(val) => val,
            EvaluatorResult::Requires(req) => return Ok(req),
        };

        if let EvaluatorValue::Bytes(b) = val {
           *result =  eval.resume_with_at_location(b)?;
           return Ok(EvalResult::Complete);
        } else {
            bail!("Error expected bytes");
        }
    }
    else {
        bail!("die has no at location");
    }
}

