pub mod value;
pub mod evaluate;
pub mod attributes;

use super::{
    call_evaluate,
};


use probe_rs::{
    MemoryInterface,
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


use evaluate::{
    parse_base_type,
};


use anyhow::{
    Result,
    anyhow,
};


pub fn evaluate<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                core:       &mut probe_rs::Core,
                unit:       &Unit<R>,
                pc:         u32,
                expr:       Expression<R>,
                frame_base: Option<u64>,
                type_unit:  Option<&gimli::Unit<R>>,
                type_die:   Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
                registers:     &Vec<(u16, u32)>,
                ) -> Result<EvaluatorValue<R>>
{
    let pieces = evaluate_pieces(dwarf, core, unit, pc, expr, frame_base, type_unit, registers)?;
    evaluate_value(dwarf, core, pieces, type_unit, type_die, registers)
}


pub fn evaluate_pieces<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                core:       &mut probe_rs::Core,
                unit:       &Unit<R>,
                pc:         u32,
                expr:       Expression<R>,
                frame_base: Option<u64>,
                type_unit:  Option<&gimli::Unit<R>>,
                registers:     &Vec<(u16, u32)>,
                ) -> Result<Vec<gimli::Piece<R>>>
{
    let mut eval    = expr.evaluation(unit.encoding());
    let mut result  = eval.evaluate()?;

    println!("fb: {:?}, pc: {:?}", frame_base, pc);
    loop {
        //println!("{:#?}", result);
        match result {
            Complete => break,
            RequiresMemory{address, size, space, base_type} =>
                resolve_requires_mem(core,
                                     unit,
                                     &mut eval,
                                     &mut result,
                                     address,
                                     size,
                                     space,
                                     base_type)?,

            RequiresRegister{register, base_type} =>
                resolve_requires_reg(core,
                                     unit,
                                     &mut eval,
                                     &mut result,
                                     register,
                                     base_type,
                                     registers)?,

            RequiresFrameBase => 
                result = eval.resume_with_frame_base(frame_base.unwrap())?, // TODO: Check and test if correct.

            RequiresTls(_tls) =>
                unimplemented!(), // TODO

            RequiresCallFrameCfa =>
                unimplemented!(), // TODO

            RequiresAtLocation(die_ref) =>
                resolve_requires_at_location(dwarf,
                                             core,
                                             unit,
                                             pc,
                                             &mut eval,
                                             &mut result,
                                             frame_base,
                                             die_ref,
                                             registers)?,

            RequiresEntryValue(e) =>
              result = eval.resume_with_entry_value(convert_to_gimli_value(evaluate(dwarf,
                                                                                                                                                        core,
                                                                  unit,
                                                                  pc,
                                                                  e,
                                                                  frame_base, 
                                                                  None,
                                                                  None,
                                                                  registers
                                                                  )?.to_value().unwrap()))?,

            RequiresParameterRef(unit_offset) => //unimplemented!(), // TODO: Check and test if correct.
                {
                    let die     = unit.entry(unit_offset)?;
                    let expr    = die.attr_value(gimli::DW_AT_call_value)?.unwrap().exprloc_value().unwrap();
                    let value   = evaluate(dwarf, core, unit, pc, expr, frame_base, type_unit, Some(&die), registers)?;

                    if let EvaluatorValue::Value(BaseValue::U64(val)) = value {
                        result = eval.resume_with_parameter_ref(val)?;
                    } else {
                        return Err(anyhow!("could not find parameter"));
                    }
                },

            RequiresRelocatedAddress(num) =>
                result = eval.resume_with_relocated_address(num)?, // TODO: Check and test if correct.

            RequiresIndexedAddress {index, relocate: _} => //unimplemented!(), // TODO: Check and test if correct. Also handle relocate flag
                result = eval.resume_with_indexed_address(dwarf.address(unit, index)?)?,

            RequiresBaseType(unit_offset) => // TODO: Check and test if correct
                result = eval.resume_with_base_type(convert_to_gimli_value(parse_base_type(unit, &[0], unit_offset)).value_type())?,
        };
    }

    let pieces = eval.result();
    println!("{:#?}", pieces);
    Ok(pieces)
}





fn resolve_requires_at_location<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                core:       &mut probe_rs::Core,
                                unit:       &Unit<R>,
                                pc:         u32,
                                eval:       &mut Evaluation<R>,
                                result:     &mut EvaluationResult<R>,
                                frame_base: Option<u64>,
                                die_ref:    DieReference<usize>,
                                registers:     &Vec<(u16, u32)>,
                                ) -> Result<()>
                                where R: Reader<Offset = usize>
{ 
    match die_ref {
        DieReference::UnitRef(unit_offset) => {
            return help_at_location(dwarf, core, unit, pc, eval, result, frame_base, unit_offset, registers);
        },

        DieReference::DebugInfoRef(debug_info_offset) => {
            let unit_header = dwarf.debug_info.header_from_offset(debug_info_offset)?;
            if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                let new_unit = dwarf.unit(unit_header)?;
                return help_at_location(dwarf, core, &new_unit, pc, eval, result, frame_base, unit_offset, registers);
            } else {
                return Err(anyhow!("Could not find at location"));
            }    
        },
    };
}


pub fn evaluate_value<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
                                                 core:       &mut probe_rs::Core,
                                                 pieces:     Vec<gimli::Piece<R>>,
                                                 type_unit:  Option<&gimli::Unit<R>>,
                                                 type_die:   Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
                                                 registers:     &Vec<(u16, u32)>,
                                                 ) -> Result<EvaluatorValue<R>>
{
    let mut evaluator = evaluate::Evaluator::new(&dwarf, pieces.clone(), type_unit, type_die);
    for (reg, data) in registers {
        evaluator.add_register(*reg, *data);
    }
    loop {
        match evaluator.evaluate(&dwarf) {
            evaluate::EvaluatorResult::Complete => break,
            evaluate::EvaluatorResult::RequireReg(reg) => { 
                println!("read reg: {:?}", reg);
                let data = core.read_core_reg(reg)?;
                evaluator.add_register(reg, data);
            },
            evaluate::EvaluatorResult::RequireData {address, num_words: _} => {
                let mut data: [u32; 1] = [0];
                core.read_32(address as u32, &mut data)?;
                evaluator.add_address(address, data[0]); // TODO: Read more then 1 word
            },
        };
    }
    let value = evaluator.get_value();

//      println!("Value: {:#?}", value);
    Ok(value.unwrap())
}


/*
 * Resolves requires memory when evaluating a die.
 * TODO: Check and test if correct.
 */
fn resolve_requires_mem<R: Reader<Offset = usize>>(core:       &mut probe_rs::Core,
                                                   unit:       &Unit<R>,
                                                   eval:       &mut Evaluation<R>,
                                                   result:     &mut EvaluationResult<R>,
                                                   address:    u64,
                                                   _size:       u8, // TODO: Handle size
                                                   _space:      Option<u64>, // TODO: Handle space
                                                   base_type:  UnitOffset<usize>
                                                   ) -> Result<()>
                                                   where R: Reader<Offset = usize>
{
    let mut data: [u32; 2] = [0,0]; // TODO: How much data should be read? 2 x 32?
    core.read_32(address as u32, &mut data)?;
    let value = parse_base_type(unit, &data, base_type);
    *result = eval.resume_with_memory(convert_to_gimli_value(value))?;    

    Ok(())
    // TODO: Mask the relevant bits?
}


/*
 * Resolves requires register when evaluating a die.
 * TODO: Check and test if correct.
 */
fn resolve_requires_reg<R: Reader<Offset = usize>>(core:       &mut probe_rs::Core,
                        unit:       &Unit<R>,
                        eval:       &mut Evaluation<R>,
                        result:     &mut EvaluationResult<R>,
                        reg:        Register,
                        base_type:  UnitOffset<usize>,
                        registers:     &Vec<(u16, u32)>,
                        ) -> Result<()>
                        where R: Reader<Offset = usize>
{
    println!("req reg: {:?}", reg.0);
    let mut data    = core.read_core_reg(reg.0)?;
    for r in registers {
        if r.0 == reg.0 {
            data = r.1;
            break;
        }
    }

    let value   = parse_base_type(unit, &[data], base_type);
    *result     = eval.resume_with_register(convert_to_gimli_value(value))?;    

    Ok(())
}


fn help_at_location<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                    core:           &mut probe_rs::Core,
                    unit:           &Unit<R>,
                    pc:             u32,
                    eval:           &mut Evaluation<R>,
                    result:         &mut EvaluationResult<R>,
                    frame_base:     Option<u64>,
                    unit_offset:    UnitOffset<usize>,
                    registers:     &Vec<(u16, u32)>,
                    ) -> Result<()>
                    where R: Reader<Offset = usize>
{
    let die = unit.entry(unit_offset)?;
    if let Some(expr) = die.attr_value(gimli::DW_AT_location)?.unwrap().exprloc_value() {
        
        let val = call_evaluate(dwarf, core, &unit, pc, expr, frame_base, &unit, &die, registers)?;

        if let EvaluatorValue::Bytes(b) = val {
           *result =  eval.resume_with_at_location(b)?;
           return Ok(());
        } else {
            return Err(anyhow!("Error expected bytes"));
        }
    }
    else {
        return Err(anyhow!("die has no at location"));
    }
}

