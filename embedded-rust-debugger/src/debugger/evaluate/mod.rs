pub mod value;
pub mod evaluate;
pub mod attributes;

use super::{
    Debugger,
    types::{
        DebuggerType,
    },
};


use probe_rs::{
    MemoryInterface,
};


use gimli::{
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
    AttributeValue::{
        Udata,
        Encoding,
    },
    Reader,
    Evaluation,
    EvaluationResult,
    UnitOffset,
    Register,
    DwAte,
    Expression,
    DieReference,
};

pub use value::{
    EvaluatorValue,
    NewStructValue,
    NewEnumValue,
    NewMemberValue,
    NewUnionValue,
    NewArrayValue,
    convert_to_gimli_value_new,
    BaseValue,
};


use anyhow::{
    Result,
    anyhow,
};


impl<R: Reader<Offset = usize>> Debugger<R> {
    pub fn evaluate(&mut self,
                    core:       &mut probe_rs::Core,
                    unit:       &Unit<R>,
                    pc:         u32,
                    expr:       Expression<R>,
                    frame_base: Option<u64>,
                    type_unit:  Option<&gimli::Unit<R>>,
                    type_die:   Option<&gimli::DebuggingInformationEntry<'_, '_, R>>
                    ) -> Result<EvaluatorValue<R>>
    {
        let mut eval    = expr.evaluation(unit.encoding());
        let mut result  = eval.evaluate()?;
    
        //println!("fb: {:?}", frame_base);
        loop {
            //println!("{:#?}", result);
            match result {
                Complete => break,
                RequiresMemory{address, size, space, base_type} =>
                    self.resolve_requires_mem(core,
                                              unit,
                                              &mut eval,
                                              &mut result,
                                              address,
                                              size,
                                              space,
                                              base_type)?,

                RequiresRegister{register, base_type} =>
                    self.resolve_requires_reg(core,
                                              unit,
                                              &mut eval,
                                              &mut result,
                                              register,
                                              base_type)?,

                RequiresFrameBase => 
                    result = eval.resume_with_frame_base(frame_base.unwrap())?, // TODO: Check and test if correct.

                RequiresTls(_tls) =>
                    unimplemented!(), // TODO

                RequiresCallFrameCfa =>
                    unimplemented!(), // TODO

                RequiresAtLocation(die_ref) =>
                    self.resolve_requires_at_location(core,
                                                      unit,
                                                      pc,
                                                      &mut eval,
                                                      &mut result,
                                                      frame_base,
                                                      die_ref)?,

                RequiresEntryValue(e) =>
                  result = eval.resume_with_entry_value(convert_to_gimli_value_new(self.evaluate(core,
                                                                      unit,
                                                                      pc,
                                                                      e,
                                                                      frame_base, 
                                                                      None,
                                                                      None
                                                                      )?.to_value().unwrap()))?,

                RequiresParameterRef(unit_offset) => //unimplemented!(), // TODO: Check and test if correct.
                    {
                        let die     = unit.entry(unit_offset)?;
                        let expr    = die.attr_value(gimli::DW_AT_call_value)?.unwrap().exprloc_value().unwrap();
                        let value   = self.evaluate(core, unit, pc, expr, frame_base, type_unit, Some(&die))?;

                        if let EvaluatorValue::Value(BaseValue::U64(val)) = value {
                            result = eval.resume_with_parameter_ref(val)?;
                        } else {
                            return Err(anyhow!("could not find parameter"));
                        }
                    },

                RequiresRelocatedAddress(num) =>
                    result = eval.resume_with_relocated_address(num)?, // TODO: Check and test if correct.

                RequiresIndexedAddress {index, relocate: _} => //unimplemented!(), // TODO: Check and test if correct. Also handle relocate flag
                    result = eval.resume_with_indexed_address(self.dwarf.address(unit, index)?)?,

                RequiresBaseType(unit_offset) => // TODO: Check and test if correct
                    result = eval.resume_with_base_type(convert_to_gimli_value_new(parse_base_type(unit, &[0], unit_offset)).value_type())?,
            };
        }
    
        let mut pieces = eval.result();
        println!("{:#?}", pieces);

        let mut evaluator = evaluate::Evaluator::new(pieces.clone(), type_unit, type_die);
        loop {
            match evaluator.evaluate(&self.dwarf) {
                evaluate::EvaluatorResult::Complete => break,
                evaluate::EvaluatorResult::RequireReg(reg) => {
                    let data = core.read_core_reg(reg)?;
                    evaluator.add_register(reg, data);
                },
                evaluate::EvaluatorResult::RequireData {address, num_words} => {
                    let mut data: [u32; 1] = [0];
                    core.read_32(address as u32, &mut data)?;
                    evaluator.add_address(address, data[0]);
                },
            };
        }
        let value = evaluator.get_value();

//        println!("Value: {:#?}", value);
        Ok(value.unwrap())
    }


    /*
     * Resolves requires memory when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_mem(&mut self,
                            core:       &mut probe_rs::Core,
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
        *result = eval.resume_with_memory(convert_to_gimli_value_new(value))?;    

        Ok(())
        // TODO: Mask the relevant bits?
    }


    /*
     * Resolves requires register when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_reg(&mut self,
                            core:       &mut probe_rs::Core,
                            unit:       &Unit<R>,
                            eval:       &mut Evaluation<R>,
                            result:     &mut EvaluationResult<R>,
                            reg:        Register,
                            base_type:  UnitOffset<usize>
                            ) -> Result<()>
                            where R: Reader<Offset = usize>
    {
        let data    = core.read_core_reg(reg.0)?;
        let value   = parse_base_type(unit, &[data], base_type);
        *result     = eval.resume_with_register(convert_to_gimli_value_new(value))?;    

        Ok(())
    }


    fn resolve_requires_at_location(&mut self,
                                    core:       &mut probe_rs::Core,
                                    unit:       &Unit<R>,
                                    pc:         u32,
                                    eval:       &mut Evaluation<R>,
                                    result:     &mut EvaluationResult<R>,
                                    frame_base: Option<u64>,
                                    die_ref:    DieReference<usize>
                                    ) -> Result<()>
                                    where R: Reader<Offset = usize>
    { 
        match die_ref {
            DieReference::UnitRef(unit_offset) => {
                return self.help_at_location(core, unit, pc, eval, result, frame_base, unit_offset);
            },

            DieReference::DebugInfoRef(debug_info_offset) => {
                let unit_header = self.dwarf.debug_info.header_from_offset(debug_info_offset)?;
                if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                    let new_unit = self.dwarf.unit(unit_header)?;
                    return self.help_at_location(core, &new_unit, pc, eval, result, frame_base, unit_offset);
                } else {
                    return Err(anyhow!("Could not find at location"));
                }    
            },
        };
    }


    fn help_at_location(&mut self,
                        core:           &mut probe_rs::Core,
                        unit:           &Unit<R>,
                        pc:             u32,
                        eval:           &mut Evaluation<R>,
                        result:         &mut EvaluationResult<R>,
                        frame_base:     Option<u64>,
                        unit_offset:    UnitOffset<usize>
                        ) -> Result<()>
                        where R: Reader<Offset = usize>
    {
        let die = unit.entry(unit_offset)?;
        if let Some(expr) = die.attr_value(gimli::DW_AT_location)?.unwrap().exprloc_value() {
            
            let val = self.call_evaluate(core, &unit, pc, expr, frame_base, &unit, &die)?;

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
}


fn parse_base_type<R>(unit:         &Unit<R>,
                      data:         &[u32],
                      base_type:    UnitOffset<usize>
                      ) -> BaseValue
                      where R: Reader<Offset = usize>
{
    if base_type.0 == 0 {
        return BaseValue::Generic(slize_as_u64(data));
    }
    let die = unit.entry(base_type).unwrap();

    // I think that the die returned must be a base type tag.
    if die.tag() != gimli::DW_TAG_base_type {
        println!("{:?}", die.tag().static_string());
        panic!("die tag not base type");
    }

    let encoding = match die.attr_value(gimli::DW_AT_encoding) {
        Ok(Some(Encoding(dwate))) => dwate,
        _ => panic!("expected Encoding"),
    };
    let byte_size = match die.attr_value(gimli::DW_AT_byte_size) {
        Ok(Some(Udata(v))) => v,
        _ => panic!("expected Udata"),
    };
    
    eval_base_type(data, encoding, byte_size)
}


pub fn eval_base_type(data:         &[u32],
                      encoding:     DwAte,
                      byte_size:    u64
                      ) -> BaseValue
{
    if byte_size == 0 {
        panic!("expected byte size to be larger then 0");
    }

    let value = slize_as_u64(data);
    match (encoding, byte_size) { 
        (DwAte(7), 1) => BaseValue::U8(value as u8),       // (unsigned, 8)
        (DwAte(7), 2) => BaseValue::U16(value as u16),     // (unsigned, 16)
        (DwAte(7), 4) => BaseValue::U32(value as u32),     // (unsigned, 32)
        (DwAte(7), 8) => BaseValue::U64(value),            // (unsigned, 64)
        
        (DwAte(5), 1) => BaseValue::I8(value as i8),       // (signed, 8)
        (DwAte(5), 2) => BaseValue::I16(value as i16),     // (signed, 16)
        (DwAte(5), 4) => BaseValue::I32(value as i32),     // (signed, 32)
        (DwAte(5), 8) => BaseValue::I64(value as i64),     // (signed, 64)

        (DwAte(2), 1) => BaseValue::Generic((value as u8) as u64), // Should be returned as bool?
        (DwAte(1), 4) => BaseValue::Address32(value as u32),
        _ => {
            //println!("{:?}, {:?}", encoding, byte_size);
            unimplemented!()
        },
    }
}

fn slize_as_u64(data: &[u32]) -> u64
{
    // TODO: Take account to what endian it is
    // TODO: Check and test if correct
    if data.len() < 2 {
        return data[0] as u64;
    }
    if data.len() > 2 {
        panic!("To big value");
    }
    return ((data[0] as u64)<< 32) + (data[1] as u64);
}

