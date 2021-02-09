use super::{
    Debugger,
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

impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn new_evaluate(&mut self,
                        unit: &Unit<R>,
                        expr: Expression<R>,
                        frame_base: Option<u64>
                    ) -> Result<Value, &'static str>
    {
        let mut eval = expr.evaluation(self.unit.encoding());
        let mut result = eval.evaluate().unwrap();
    
        println!("fb: {:?}", frame_base);
        loop {
            println!("{:#?}", result);
            match result {
                Complete => break,
                RequiresMemory{address, size, space, base_type} =>
                    self.resolve_requires_mem(unit, &mut eval, &mut result, address, size, space, base_type),
                RequiresRegister{register, base_type} => self.resolve_requires_reg(unit, &mut eval, &mut result, register, base_type),
                RequiresFrameBase => 
                    result = eval.resume_with_frame_base(frame_base.unwrap()).unwrap(), // TODO: Check and test if correct.
                RequiresTls(_tls) => unimplemented!(), // TODO
                RequiresCallFrameCfa => unimplemented!(), // TODO
                RequiresAtLocation(die_ref) => self.resolve_requires_at_location(unit, &mut eval, &mut result, frame_base, die_ref)?,
                RequiresEntryValue(e) =>
                  result = eval.resume_with_entry_value(self.new_evaluate(unit, e, frame_base)?).unwrap(),
                RequiresParameterRef(unit_offset) => //unimplemented!(), // TODO: Check and test if correct.
                    {
                        let die = unit.entry(unit_offset).unwrap();
                        let expr = die.attr_value(gimli::DW_AT_call_value).unwrap().unwrap().exprloc_value().unwrap();
                        let value = self.new_evaluate(unit, expr, frame_base).unwrap();
                        if let Value::U64(val) = value {
                            result = eval.resume_with_parameter_ref(val).unwrap();
                        } else {
                            return Err("could not find parameter");
                        }
                    },
                RequiresRelocatedAddress(num) =>
                    result = eval.resume_with_relocated_address(num).unwrap(), // TODO: Check and test if correct.
                RequiresIndexedAddress {index, relocate} => //unimplemented!(), // TODO: Check and test if correct. Also handle rolocate flag
                    result = eval.resume_with_indexed_address(self.dwarf.address(unit, index).unwrap()).unwrap(),
                RequiresBaseType(unit_offset) => 
                    result = eval.resume_with_base_type(
                        parse_base_type(unit, 0, unit_offset).value_type()).unwrap(),
            };
        }
    
        let value = self.eval_pieces(eval.result());
        println!("Value: {:?}", value);
        value
    }


    fn eval_pieces(&mut self,
                   pieces: Vec<Piece<R>>
                   ) -> Result<Value, &'static str>
    {
        // TODO: What should happen if more then one piece is given?
        if pieces.len() > 1 {
            panic!("Found more then one piece");
        }
        println!("{:?}", pieces);
        return self.eval_piece(&pieces[0]);
    }
    
    fn eval_piece(&mut self,
                  piece: &Piece<R>
                  ) -> Result<Value, &'static str>
    {
        // TODO: Handle size_in_bits and bit_offset
        match &piece.location {
            Location::Empty => return Err("Optimized out"),
            Location::Register { register } => // TODO Always return U32?
                return Ok(Value::U32(self.core.read_core_reg(register.0).unwrap())),
            Location::Address { address } =>  // TODO Always return U32?
                return Ok(Value::U32(self.core.read_word_32(*address as u32).map_err(|e| "Read error")?)),
            Location::Value { value } => return Ok(value.clone()),
            Location::Bytes { value } => unimplemented!(), // TODO
            Location::ImplicitPointer { value, byte_offset } => unimplemented!(), // TODO
        };
    }

    /*
     * Resolves requires memory when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_mem(&mut self,
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            address: u64,
            size: u8,
            _space: Option<u64>,
            base_type: UnitOffset<usize>
        )
            where R: Reader<Offset = usize>
    {
        let data = self.core.read_word_32(address as u32).unwrap();
        let value = parse_base_type(unit, data, base_type);
        *result = eval.resume_with_memory(value).unwrap();    
    }


    /*
     * Resolves requires register when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_reg(&mut self,
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            reg: Register,
            base_type: UnitOffset<usize>
        ) 
            where R: Reader<Offset = usize>
    {
        let data = self.core.read_core_reg(reg.0).unwrap();
        let value = parse_base_type(unit, data, base_type);
        *result = eval.resume_with_register(value).unwrap();    
    }

    fn resolve_requires_at_location(&mut self,
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            frame_base: Option<u64>,
            die_ref: DieReference<usize>
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
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            frame_base: Option<u64>,
            unit_offset: UnitOffset<usize>
        ) -> Result<(), &'static str>
            where R: Reader<Offset = usize>
    {
        let die = unit.entry(unit_offset).unwrap();
        if let Some(expr) = die.attr_value(gimli::DW_AT_location).unwrap().unwrap().exprloc_value() {
    
            let val = self.new_evaluate(&unit, expr, frame_base);
            unimplemented!(); // TODO: Add a value enum
    //          eval.resume_with_at_location(val.bytes); // val need to be of type bytes: R
        }
        else {
            return Err("die has no at location");
        }
    }

}




fn parse_base_type<R>(
        unit: &Unit<R>,
        data: u32,
        base_type: UnitOffset<usize>
    ) -> Value
        where R: Reader<Offset = usize>
{
    if base_type.0 == 0 {
        return Value::Generic(data as u64);
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
    
    // Check dwarf doc for the codes.
    match (encoding, byte_size) {
        (DwAte(7), 1) => Value::U8(data as u8),     // (unsigned, 8)
        (DwAte(7), 2) => Value::U16(data as u16),   // (unsigned, 16)
        (DwAte(7), 4) => Value::U32(data as u32),   // (unsigned, 32)
        (DwAte(7), 8) => Value::U64(data as u64),   // (unsigned, 64) TODO: Fix
        
        (DwAte(5), 1) => Value::I8(data as i8),     // (signed, 8)
        (DwAte(5), 2) => Value::I16(data as i16),   // (signed, 16)
        (DwAte(5), 4) => Value::I32(data as i32),   // (signed, 32)
        (DwAte(5), 8) => Value::I64(data as i64),   // (signed, 64) TODO: Fix
        _ => unimplemented!(),
    }
}

