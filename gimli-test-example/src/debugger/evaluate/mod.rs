pub mod value;
pub mod evaluate;

use super::{
    Debugger,
    types::{
        DebuggerType,
        TypeInfo,
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
    Value,
    DwAte,
    Expression,
    Piece,
    Location,
    DieReference,
};

pub use value::{
    DebuggerValue,
    StructValue,
    EnumValue,
    MemberValue,
    UnionValue,
};


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn evaluate(&mut self,
                    unit:       &Unit<R>,
                    expr:       Expression<R>,
                    frame_base: Option<u64>,
                    vtype:      Option<&DebuggerType>,
                    ) -> Result<DebuggerValue<R>, &'static str>
    {
        let mut eval    = expr.evaluation(self.unit.encoding());
        let mut result  = eval.evaluate().unwrap();
    
        println!("fb: {:?}", frame_base);
        loop {
            println!("{:#?}", result);
            match result {
                Complete => break,
                RequiresMemory{address, size, space, base_type} =>
                    self.resolve_requires_mem(unit,
                                              &mut eval,
                                              &mut result,
                                              address,
                                              size,
                                              space,
                                              base_type),

                RequiresRegister{register, base_type} =>
                    self.resolve_requires_reg(unit,
                                              &mut eval,
                                              &mut result,
                                              register,
                                              base_type),

                RequiresFrameBase => 
                    result = eval.resume_with_frame_base(frame_base.unwrap()).unwrap(), // TODO: Check and test if correct.

                RequiresTls(_tls) =>
                    unimplemented!(), // TODO

                RequiresCallFrameCfa =>
                    unimplemented!(), // TODO

                RequiresAtLocation(die_ref) =>
                    self.resolve_requires_at_location(unit,
                                                      &mut eval,
                                                      &mut result,
                                                      frame_base,
                                                      die_ref)?,

                RequiresEntryValue(e) =>
                  result = eval.resume_with_entry_value(self.evaluate(unit, e, frame_base, None)?.to_value().unwrap()).unwrap(),

                RequiresParameterRef(unit_offset) => //unimplemented!(), // TODO: Check and test if correct.
                    {
                        let die     = unit.entry(unit_offset).unwrap();
                        let dtype   = self.type_attribute(&die).unwrap();
                        let expr    = die.attr_value(gimli::DW_AT_call_value).unwrap().unwrap().exprloc_value().unwrap();
                        let value   = self.evaluate(unit, expr, frame_base, Some(&dtype)).unwrap();

                        if let DebuggerValue::Value(Value::U64(val)) = value {
                            result = eval.resume_with_parameter_ref(val).unwrap();
                        } else {
                            return Err("could not find parameter");
                        }
                    },

                RequiresRelocatedAddress(num) =>
                    result = eval.resume_with_relocated_address(num).unwrap(), // TODO: Check and test if correct.

                RequiresIndexedAddress {index, relocate} => //unimplemented!(), // TODO: Check and test if correct. Also handle rolocate flag
                    result = eval.resume_with_indexed_address(self.dwarf.address(unit, index).unwrap()).unwrap(),

                RequiresBaseType(unit_offset) => // TODO: Check and test if correct
                    result = eval.resume_with_base_type(
                        parse_base_type(unit, &[0], unit_offset).value_type()).unwrap(),
            };
        }
    
        println!("Type: {:#?}", vtype);
        let mut pieces = eval.result();
        println!("{:#?}", pieces);
        let value =  match vtype {
            Some(t) => self.eval_type(&mut pieces, t),
            None => self.eval_piece(pieces.remove(0), None, None),
        };
        println!("Value: {:#?}", value);
        Ok(value.unwrap())
    }



    //fn eval_pieces(&mut self,
    //               pieces:  Vec<Piece<R>>,
    //               vtype:   Option<&DebuggerType>
    //               ) -> Result<DebuggerValue<R>, &'static str>
    //{
    //    println!("{:#?}", pieces);
    //    // TODO: What should happen if more then one piece is given?
    //    if pieces.len() > 1 {
    //        for p in &pieces {
    //            println!("Value {:#?}", self.eval_piece(p, vtype)); //TODO
    //        }
    //        //panic!("Found more then one piece");
    //    }
    //    return self.eval_piece(&pieces[0], vtype); //TODO
    //}
   

    //fn eval_piece(&mut self,
    //              piece: &Piece<R>,
    //              vtype: Option<&DebuggerType>
    //              ) -> Result<DebuggerValue<R>, &'static str>
    //{
    //    fn parse_value<R: Reader<Offset = usize>>(data:         u32,
    //                                              size_in_bits: Option<u64>,
    //                                              bit_offset:   Option<u64>
    //                                              ) -> Result<DebuggerValue<R>, &'static str>
    //    {
    //        let mut mask: u32 = u32::MAX;
    //        if let Some(bits) = size_in_bits {
    //            if bits > 32 {
    //                return Err("not enough bits");
    //            }
    //            mask = mask >> (32 - bits);
    //        }
    //        if let Some(offset) = bit_offset {
    //            if offset >= 32 {
    //                return Err("not enough bits");
    //            }
    //            mask = mask << offset;
    //        }
    //        return Ok(DebuggerValue::Value(Value::U32(data & mask))); // TODO: Always return U32?
    //    }

    //    let reg_size: u64 = match vtype {
    //        Some(dtype) => 1,//(dtype.byte_size() + 4 - 1)/4,
    //        None        => 1,
    //    };
    //    match &piece.location {
    //        Location::Empty =>
    //            return Ok(DebuggerValue::Non), //return Err("Optimized out"),

    //        Location::Register { register } => 
    //            return parse_value(self.core.read_core_reg(register.0).unwrap(),
    //                               piece.size_in_bits,
    //                               piece.bit_offset),

    //        Location::Address { address } => { //TODO:
    //            //let address = match vtype {
    //            //    Some(vt) => address + (address%(match vt.alignment() {Some(v) => v, None => 1,})),
    //            //    None => *address,
    //            //};
    //            //println!("address: {:?}", address);

    //            let mut data: Vec<u32> = vec![0; reg_size as usize];
    //            self.core.read_32(*address as u32, &mut data).map_err(|e| "Read error")?;
    //            let mut res: Vec<u32> = Vec::new();
    //            for d in data.iter() {
    //                res.push(*d);
    //            }
    //            println!("Raw: {:?}", res);
    //            return Ok(DebuggerValue::Raw(res));
    //            //match vtype {
    //            //    Some(t) => {
    //            //        return match self.parse_value(res.clone(), vtype.unwrap()) {
    //            //            Ok(val) => return Ok(val),
    //            //            Err(_)  => Ok(DebuggerValue::Raw(res)),
    //            //        } //TODO: Uncomment
    //            //    },
    //            //    None => return Ok(DebuggerValue::Raw(res)),
    //            //};
    //        },

    //        Location::Value { value } => {
    //            //if let Some(_) = piece.size_in_bits {
    //            //    panic!("needs to be implemented");
    //            //}
    //            //if let Some(_) = piece.bit_offset {
    //            //    panic!("needs to be implemented");
    //            //}
    //            return Ok(DebuggerValue::Value(value.clone()));
    //        }, // TODO: Handle size_in_bits and bit_offset?

    //        Location::Bytes { value } => // TODO: Check and test if correct
    //            return Ok(DebuggerValue::Bytes(value.clone())),

    //        Location::ImplicitPointer { value, byte_offset } => unimplemented!(), // TODO
    //    };
    //}


    /*
     * Resolves requires memory when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_mem(&mut self,
                            unit:       &Unit<R>,
                            eval:       &mut Evaluation<R>,
                            result:     &mut EvaluationResult<R>,
                            address:    u64,
                            size:       u8, // TODO: Handle size
                            space:      Option<u64>, // TODO: Handle space
                            base_type:  UnitOffset<usize>
                            )
                            where R: Reader<Offset = usize>
    {
        let mut data: [u32; 2] = [0,0]; // TODO: How much data should be read? 2 x 32?
        self.core.read_32(address as u32, &mut data).unwrap();
        let value = parse_base_type(unit, &data, base_type);
        *result = eval.resume_with_memory(value).unwrap();    
        // TODO: Mask the relavent bits?
    }


    /*
     * Resolves requires register when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_reg(&mut self,
                            unit:       &Unit<R>,
                            eval:       &mut Evaluation<R>,
                            result:     &mut EvaluationResult<R>,
                            reg:        Register,
                            base_type:  UnitOffset<usize>
                            ) 
                            where R: Reader<Offset = usize>
    {
        let data    = self.core.read_core_reg(reg.0).unwrap();
        let value   = parse_base_type(unit, &[data], base_type);
        *result     = eval.resume_with_register(value).unwrap();    
    }


    fn resolve_requires_at_location(&mut self,
                                    unit:       &Unit<R>,
                                    eval:       &mut Evaluation<R>,
                                    result:     &mut EvaluationResult<R>,
                                    frame_base: Option<u64>,
                                    die_ref:    DieReference<usize>
                                    ) -> Result<(), &'static str>
                                    where R: Reader<Offset = usize>
    { 
        match die_ref {
            DieReference::UnitRef(unit_offset) => {
                return self.help_at_location(unit, eval, result, frame_base, unit_offset);
            },

            DieReference::DebugInfoRef(debug_info_offset) => {
                let unit_header = self.dwarf.debug_info.header_from_offset(debug_info_offset).map_err(|_| "Can't find debug info header")?;
                if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                    let new_unit = self.dwarf.unit(unit_header).map_err(|_| "Can't find unit using unit header")?;
                    return self.help_at_location(&new_unit, eval, result, frame_base, unit_offset);
                } else {
                    return Err("Could not find at location");
                }    
            },
        };
    }


    fn help_at_location(&mut self,
                        unit:           &Unit<R>,
                        eval:           &mut Evaluation<R>,
                        result:         &mut EvaluationResult<R>,
                        frame_base:     Option<u64>,
                        unit_offset:    UnitOffset<usize>
                        ) -> Result<(), &'static str>
                        where R: Reader<Offset = usize>
    {
        let die = unit.entry(unit_offset).unwrap();
        if let Some(expr) = die.attr_value(gimli::DW_AT_location).unwrap().unwrap().exprloc_value() {
            
            let dtype   = self.type_attribute(&die).unwrap();
            let val     = self.evaluate(&unit, expr, frame_base, Some(&dtype))?;

            if let DebuggerValue::Bytes(b) = val {
               *result =  eval.resume_with_at_location(b).unwrap();
               return Ok(());
            } else {
                panic!("Error expected bytes");
            }
        }
        else {
            return Err("die has no at location");
        }
    }
}


fn parse_base_type<R>(unit:         &Unit<R>,
                      data:         &[u32],
                      base_type:    UnitOffset<usize>
                      ) -> Value
                      where R: Reader<Offset = usize>
{
    if base_type.0 == 0 {
        return Value::Generic(slize_as_u64(data));
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
                      ) -> Value
{
    if byte_size == 0 {
        panic!("expected byte size to be larger then 0");
    }

    let value = slize_as_u64(data);
    match (encoding, byte_size) { 
        (DwAte(7), 1) => Value::U8(value as u8),       // (unsigned, 8)
        (DwAte(7), 2) => Value::U16(value as u16),     // (unsigned, 16)
        (DwAte(7), 4) => Value::U32(value as u32),     // (unsigned, 32)
        (DwAte(7), 8) => Value::U64(value),            // (unsigned, 64)
        
        (DwAte(5), 1) => Value::I8(value as i8),       // (signed, 8)
        (DwAte(5), 2) => Value::I16(value as i16),     // (signed, 16)
        (DwAte(5), 4) => Value::I32(value as i32),     // (signed, 32)
        (DwAte(5), 8) => Value::I64(value as i64),     // (signed, 64)

        (DwAte(2), 1) => Value::Generic((value as u8) as u64), // Should be returnd as bool?
        _ => {
            println!("{:?}, {:?}", encoding, byte_size);
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

