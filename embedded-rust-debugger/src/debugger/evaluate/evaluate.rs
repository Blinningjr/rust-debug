/*
 * TODO: Consider attributes:
 *
 *      Attributes for evaluating value:
 *          DW_AT_alignment                 (Think I have implemented it correctly)(TODO: Confirm
 *          that this is correct or improve the solution)
 *          DW_AT_const_value               (Implemented for `DW_TAG_enumerator`)(TODO: Implement
 *          for constant variables)
 *          DW_AT_count                     (Done)
 *          DW_AT_data_member_location      (Maybe)
 *          DW_AT_encoding                  (Done)(Uses encoding when it is given for all of the
 *          `eval_piece` cases)
 *          DW_AT_discr                     (Done)(Implemented for DW_TAG_variant_part)
 *          DW_AT_discr_value               (Done)(Implemented for DW_TAG_variant)
 *          DW_AT_discr_list                (Not Implemented) // NOTE: Missing discr value means
 *          that it is a default variant.
 *          DW_AT_enum_class                (Can ignore)(Flag for languages with multiple enum
 *          defenitions?)
 *          DW_AT_lower_bound
 *          DW_AT_upper_bound
 *
 *      
 *      Function call information:
 *          DW_AT_call_column
 *          DW_AT_call_file
 *          DW_AT_call_line
 *
 *      DW_AT_artificial                    (I think that this is not needed)(TODO: Confirm)
 *
 *      DW_AT_containing_type               (I think that this is not needed)(TODO: Confirm) NOTE:
 *      I do not fully understand this attribute.
 */


use super::{
    attributes,
    value::{
        BaseValue,
        PartialValue,
        EvaluatorValue,
    },
};


use gimli::{
    Reader,
    Piece,
    Location,
    DwAte,
};


use anyhow::{
    anyhow,
    Result,
};


/*
 * The state of a partially evaluated type.
 */
#[derive(Debug)]
struct EvaluatorState<R: Reader<Offset = usize>> {
    pub unit_offset:    gimli::UnitSectionOffset,
    pub die_offset:     gimli::UnitOffset,
    pub partial_value:  super::value::PartialValue<R>,
    pub data_offset:    u64,
}


impl<R: Reader<Offset = usize>> EvaluatorState<R> {
    pub fn new(dwarf:       &gimli::Dwarf<R>,
               unit:        &gimli::Unit<R>,
               die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
               data_offset: u64
               ) -> EvaluatorState<R>
    {
        let partial_value = match die.tag() { 
            gimli::DW_TAG_array_type => {
                super::value::PartialValue::Array(Box::new(super::value::PartialArrayValue { count: None, values: vec!() }))
            },
            gimli::DW_TAG_structure_type => {
                let name = attributes::name_attribute(dwarf, die).unwrap();
                super::value::PartialValue::Struct(Box::new(super::value::PartialStructValue { name: name, members: vec!() }))
            },
            gimli::DW_TAG_union_type => {
                let name = attributes::name_attribute(dwarf, die).unwrap();
                super::value::PartialValue::Union(Box::new(super::value::PartialUnionValue { name: name, members: vec!() }))
            },
            gimli::DW_TAG_variant_part => {
                super::value::PartialValue::VariantPart(super::value::PartialVariantPartValue { variant: None }) 
            }, 
            _ => PartialValue::NotEvaluated,
        };

        EvaluatorState {
            unit_offset:    unit.header.offset(),
            die_offset:     die.offset(),
            partial_value:  partial_value,
            data_offset:    data_offset,
        }
    }
}


/*
 * The result of the evaluation.
 */
pub enum EvaluatorResult {
    // Evaluator has evaluated the type into a value.
    Complete,
    // Evaluator requires the value of a register.
    RequireReg(u16),
    // Evaluator requires the value of a address.
    RequireData {address: u32, num_words: usize},
}


/*
 * Internal result struct that show if a value is reached or if a value is required.
 */
pub enum ReturnResult<R: Reader<Offset = usize>> {
    Value(super::value::EvaluatorValue<R>),
    Required(EvaluatorResult),
}


/*
 * Evaluates the value of a type given Dwarf pieces.
 */
pub struct Evaluator<R: Reader<Offset = usize>> {
    pieces:         Vec<Piece<R>>,
    piece_index:    usize,
    stack:          Vec<EvaluatorState<R>>,
    result:         Option<super::value::EvaluatorValue<R>>,
    registers:      std::collections::HashMap<u16, u32>,
    addresses:      std::collections::HashMap<u32, u32>,
}


impl<R: Reader<Offset = usize>> Evaluator<R> {
    pub fn new(dwarf:   &gimli::Dwarf<R>,
               pieces:  Vec<Piece<R>>,
               unit:    Option<&gimli::Unit<R>>,
               die:     Option<&gimli::DebuggingInformationEntry<'_, '_, R>>
               ) -> Evaluator<R>
    {
        // If no unit and die is given then the first piece will be evaluated.
        let stack = match unit {
            Some(u) => {
                match die {
                    Some(d) => vec!(EvaluatorState::new(dwarf, u, d, 0)),
                    None => vec!(),
                }
            },
            None => vec!(),
        };
        Evaluator {
            pieces:         pieces,
            piece_index:    0,
            stack:          stack,
            result:         None,
            registers:      std::collections::HashMap::new(),
            addresses:      std::collections::HashMap::new(),
        }
    }

    pub fn add_address(&mut self, address: u32, value: u32) {
        self.addresses.insert(address, value);
    }

    pub fn add_register(&mut self, register: u16, value: u32) {
        self.registers.insert(register, value);
    }


    pub fn evaluate(&mut self, dwarf: &gimli::Dwarf<R>) -> EvaluatorResult {
        // If the value has already been evaluated then don't evaluated it again.
        if self.result.is_some() {
            return EvaluatorResult::Complete;
        }

        // If the stack is empty then the first piece will be evaluated.
        if self.stack.len() == 0 {
            match self.eval_piece(self.pieces[0].clone(), Some(4), 0, Some(DwAte(1))).unwrap() {
                ReturnResult::Value(val) => {
                    self.result = Some(val);
                    return EvaluatorResult::Complete;
                },
                ReturnResult::Required(req) => return req,
            };
        } 

        // Loop through the stack until it is empty because then the value is found.
        let mut result = None;
        loop {
            //println!("eval stack len: {:#?}", self.stack.len());

            // If the stack is empty then the current result should be correct value.
            if self.stack.len() == 0 {
                self.result = result;
                return EvaluatorResult::Complete;
            }

            // Get the current state.
            let (unit_offset, die_offset, data_offset) = {
                let state = &self.stack[self.stack.len() - 1];
                (state.unit_offset, state.die_offset, state.data_offset)
            };

        
            // Get the unit of the current state.
            let unit = match unit_offset {
                gimli::UnitSectionOffset::DebugInfoOffset(offset) => {
                    let header = dwarf.debug_info.header_from_offset(offset).unwrap();
                    dwarf.unit(header).unwrap()
                },
                gimli::UnitSectionOffset::DebugTypesOffset(_offset) => {
                    let mut iter = dwarf.debug_types.units();
                    let mut result = None;
                    while let Some(header) = iter.next().unwrap() {
                        if header.offset() == unit_offset {
                            result = Some(dwarf.unit(header).unwrap());
                            break;
                        }
                    }
                    result.unwrap()
                },
            };

            // Get the die of the current state.
            let die = &unit.entry(die_offset).unwrap();
            //println!("die tag {:?}", die.tag().static_string());

            // Continue evaluating the value of the current state.
            match self.eval_type(dwarf, &unit, die, data_offset, result, false).unwrap().unwrap() {
                ReturnResult::Value(val) => result = Some(val),
                ReturnResult::Required(req) => return req,
            };
        }
    }


    /*
     * Get the result of the evaluator.
     */
    pub fn get_value(self) -> Option<super::value::EvaluatorValue<R>> {
        self.result
    }


    /*
     * Helper method for getting the unit and die from the type attribute of the current die.
     */
    fn get_type_info(&mut self,
                         dwarf: &gimli::Dwarf<R>,
                         unit:  &gimli::Unit<R>,
                         die:   &gimli::DebuggingInformationEntry<'_, '_, R>,
                         ) -> Result<(gimli::Unit<R>, gimli::UnitOffset)>
    {
        let (unit_offset, die_offset) = attributes::type_attribute(dwarf, unit, die).unwrap();
        let unit = match unit_offset {
            gimli::UnitSectionOffset::DebugInfoOffset(offset) => {
                let header = dwarf.debug_info.header_from_offset(offset)?;
                dwarf.unit(header)?
            },
            gimli::UnitSectionOffset::DebugTypesOffset(_offset) => {
                let mut iter = dwarf.debug_types.units();
                let mut result = None;
                while let Some(header) = iter.next()? {
                    if header.offset() == unit_offset {
                        result = Some(dwarf.unit(header)?);
                        break;
                    }
                }
                result.unwrap()
            },
        };
       
        Ok((unit, die_offset))
    }

/*
     * Evaluates the value of a type.
     */
    pub fn eval_type(&mut self,
                     dwarf:         &gimli::Dwarf<R>,
                     unit:          &gimli::Unit<R>,
                     die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                     data_offset:   u64,
                     old_result:    Option<EvaluatorValue<R>>,
                     create_state:  bool,
                     ) -> Result<Option<ReturnResult<R>>>
    { 
        match die.tag() {
            gimli::DW_TAG_base_type                 => self.eval_basetype(dwarf, unit, die, data_offset, create_state),
            gimli::DW_TAG_pointer_type              => self.eval_pointer_type(dwarf, unit, die, data_offset, old_result, create_state),
            gimli::DW_TAG_array_type                => self.eval_array_type(dwarf, unit, die, data_offset, old_result, create_state),
            gimli::DW_TAG_structure_type            => self.eval_structured_type(dwarf, unit, die, data_offset, old_result, create_state),
            gimli::DW_TAG_union_type                => self.eval_union_type(dwarf, unit, die, data_offset, old_result, create_state),
            gimli::DW_TAG_member                    => self.eval_member(dwarf, unit, die, data_offset, old_result, create_state),
            gimli::DW_TAG_enumeration_type          => self.eval_enumeration_type(dwarf, unit, die, data_offset, old_result, create_state),
            gimli::DW_TAG_string_type               => unimplemented!(),
            gimli::DW_TAG_generic_subrange          => unimplemented!(),
            gimli::DW_TAG_template_type_parameter   => unimplemented!(),
            gimli::DW_TAG_variant_part              => self.eval_variant_part(dwarf, unit, die, data_offset, old_result, create_state),
            gimli::DW_TAG_subroutine_type           => unimplemented!(),
            gimli::DW_TAG_subprogram                => unimplemented!(),
            _ => unimplemented!(),
        }
    }


    /*
     * Evaluate the value of a piece.
     */
    pub fn eval_piece(&mut self,
                      piece:        Piece<R>,
                      byte_size:    Option<u64>,
                      data_offset:  u64,            // TODO: Maby use data offset to know which part to mask?
                      encoding:     Option<DwAte>
                      ) -> Option<ReturnResult<R>>
    {
        return match piece.location {
            Location::Empty                                         => Some(ReturnResult::Value(super::value::EvaluatorValue::OptimizedOut)),
            Location::Register        { register }                  => self.eval_register(register, byte_size, encoding),
            Location::Address         { address }                   => self.eval_address(address, byte_size, data_offset, encoding.unwrap()),
            Location::Value           { value }                     => self.eval_gimli_value(value, byte_size, encoding),
            Location::Bytes           { value }                     => Some(ReturnResult::Value(super::value::EvaluatorValue::Bytes(value))),
            Location::ImplicitPointer { value: _, byte_offset: _ }  => unimplemented!(),
        };
    }

    pub fn eval_gimli_value(&mut self,
                         value:     gimli::Value,
                         byte_size: Option<u64>,
                         encoding:  Option<DwAte>,
                         ) -> Option<ReturnResult<R>>
    {
        match (value, encoding) {
            (gimli::Value::Generic(val), Some(dwate)) => { // NOTE: Don't know if this is correct.
                let values = vec!((val >> 32) as u32, val as u32);
                Some(ReturnResult::Value(
                        EvaluatorValue::Value(
                            eval_base_type(&values, dwate, match byte_size {Some(v) => v, None => 8,}))))
            },
            _ => Some(ReturnResult::Value(
                    EvaluatorValue::Value(
                        super::value::convert_from_gimli_value(value)))),
        }
    }


    /*
     * Evaluate the value of a register.
     */
    pub fn eval_register(&mut self,
                         register:  gimli::Register,
                         byte_size: Option<u64>,
                         encoding:  Option<DwAte>,
                         ) -> Option<ReturnResult<R>>
    {
        match self.registers.get(&register.0) {
            Some(val) => { // TODO: Mask the important bits?
                match encoding {
                    Some(dwate) => Some(ReturnResult::Value( // NOTE: Don't know if this is correct.
                            super::value::EvaluatorValue::Value(
                                eval_base_type(&[*val], dwate, match byte_size {Some(v) => v, None => 4,})))),
                    None => Some(ReturnResult::Value(
                            super::value::EvaluatorValue::Value(
                                BaseValue::U32(*val)))),
                }
            },
            None    => Some(ReturnResult::Required(
                    EvaluatorResult::RequireReg(
                        register.0))),
        }
    }


    /*
     * Evaluate the value of a address.
     */
    pub fn eval_address(&mut self,
                        mut address:    u64,
                        byte_size:      Option<u64>,
                        data_offset:    u64,
                        encoding:       DwAte
                        ) -> Option<ReturnResult<R>>
    {
        let num_words = match byte_size {
            Some(val)   => (val + 4 - 1 )/4,
            None        => 1,
        };

        //println!("Address: {:#10x}", address);
        //println!("data_offset: {}", data_offset);
        address += (data_offset/4) * 4;
        //println!("Address: {:#10x}", address);

        address -= address%4; // TODO: Is this correct?


        let mut data: Vec<u32> = Vec::new();
        for i in 0..num_words as usize {
            match self.addresses.get(&((address + (i as u64) * 4) as u32)) {
                Some(val) => data.push(*val), // TODO: Mask the important bits?
                None    => return Some(ReturnResult::Required(EvaluatorResult::RequireData{ address: (address + (i as u64) * 4) as u32, num_words: 1 })),
            }
        }

        Some(ReturnResult::Value(
                super::value::EvaluatorValue::Value(
                    eval_base_type(&data, encoding, byte_size.unwrap()))))
    }


    /*
     * Evaluates the value of a piece and decides if the piece should be discarded or kept.
     */
    pub fn handle_eval_piece(&mut self,
                             byte_size:         Option<u64>,
                             mut data_offset:   u64,
                             encoding:          Option<DwAte>
                             ) -> Result<Option<ReturnResult<R>>>
    {
        if self.pieces.len() <= self.piece_index {
            return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::OptimizedOut)));
        }
        
        if self.pieces.len() > 1 { // NOTE: Is this correct?
            data_offset = 0;
        }
 
        // Evaluate the value of the piece.
        let res = self.eval_piece(self.pieces[self.piece_index].clone(),
                                  byte_size,
                                  data_offset,
                                  encoding);

        // Pops piece if the value was evaluated.
        match res.unwrap() {
            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
            ReturnResult::Value(value) => {
                match self.pieces[self.piece_index].size_in_bits {
                    Some(val)   => {
                        let bytes: i32 = match byte_size {
                            Some(val)   => (val*8) as i32,
                            None        => 32,
                        };

                        if (val as i32) - bytes < 1 {
                            self.pieces[self.piece_index].size_in_bits = Some(0);
                            self.piece_index += 1;
                        } else {
                            self.pieces[self.piece_index].size_in_bits = Some(val - bytes as u64);
                        }
                    },
                    None        => (),
                }

                return Ok(Some(ReturnResult::Value(value)));
            },
        };
    }


    /*
     * Evaluate the value of a base type.
     */
    pub fn eval_basetype(&mut self,
                         dwarf:         &gimli::Dwarf<R>,
                         unit:          &gimli::Unit<R>,
                         die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                         data_offset:   u64,
                         new_state:     bool,
                         ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has the tag DW_TAG_base_type.
        match die.tag() {
            gimli::DW_TAG_base_type => (),
            _ => panic!("Wrong implementation"),
        };

        if new_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
        }
        
        self.check_alignment(die, data_offset)?;

        // Get byte size and encoding from the die.
        let byte_size = attributes::byte_size_attribute(die);
        let encoding =  attributes::encoding_attribute(die);
        match byte_size {
            // If the byte size is 0 then the value is optimized out.
            Some(0) => {
                self.stack.pop();
                return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::ZeroSize)));
            },
            _       => (),
        };

        // Evaluate the value.
        match self.handle_eval_piece(byte_size,
                                     data_offset, // TODO
                                     encoding)?.unwrap()
        {
            ReturnResult::Value(val) => {
                self.stack.pop();
                return Ok(Some(ReturnResult::Value(val)));
            },
            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
        };
    }


    /*
     * Evaluate the value of a pointer type.
     */
    pub fn eval_pointer_type(&mut self,
                             dwarf:         &gimli::Dwarf<R>,
                             unit:          &gimli::Unit<R>,
                             die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                             data_offset:   u64,
                             old_result:    Option<EvaluatorValue<R>>,
                             create_state:  bool,
                             ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has the tag DW_TAG_array_type.
        match die.tag() {
            gimli::DW_TAG_pointer_type => (),
            _ => panic!("Wrong implementation"),
        };
        
        // Create a new state if it doesn't already exist.
        if create_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
        }

        // Use already evaluated value.
        match old_result {
            Some(val) => {
                self.stack.pop();
                return Ok(Some(ReturnResult::Value(val)));
            },
            None => (),
        };
        
        self.check_alignment(die, data_offset)?;

        // Evaluate the pointer type value.
        let address_class = attributes::address_class_attribute(die);
        match address_class.unwrap().0 {
            0 => {
                let res = self.handle_eval_piece(Some(4),
                                                 data_offset,
                                                 Some(DwAte(1)));
                self.stack.pop();
                return res;        
            },
            _ => panic!("Unimplemented DwAddr code"), // NOTE: The codes are architecture specific.
        };
    }


    /*
     * Evaluate the value of a array type.
     */
    pub fn eval_array_type(&mut self,
                           dwarf:       &gimli::Dwarf<R>,
                           unit:        &gimli::Unit<R>,
                           die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                           data_offset: u64,
                           mut old_result:  Option<EvaluatorValue<R>>,
                           new_state:   bool
                           ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has the tag DW_TAG_array_type.
        match die.tag() {
            gimli::DW_TAG_array_type => (),
            _ => panic!("Wrong implementation"),
        };

        // Get the index of the current state.
        let mut current_state = self.stack.len() - 1;

        // Create a new state if it doesn't already exist.
        if new_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
            current_state += 1;
        } 

        self.check_alignment(die, data_offset)?;

        // Get the partial array value from the current state.
        let mut partial_array = match &self.stack[current_state].partial_value {
            super::value::PartialValue::Array   (array) => array.clone(),
            _ => return Err(anyhow!("Critical Error: expected partial array")),
        };
       
        // Evaluate the length of the array.
        let count = match partial_array.count {
            Some(val)   => val,
            None        => {
                let children = get_children(unit, die);
                let dimension_die = unit.entry(children[0])?;
                let value = match old_result {
                    Some(val)   => {
                        old_result = None;
                        val
                    },
                    None    => {
                        let result = match dimension_die.tag() {
                            gimli::DW_TAG_subrange_type     => self.eval_subrange_type(dwarf, unit, &dimension_die, data_offset, old_result.clone())?.unwrap(),
                            gimli::DW_TAG_enumeration_type  => self.eval_enumeration_type(dwarf, unit, &dimension_die, data_offset, old_result.clone(), true)?.unwrap(),
                            _ => unimplemented!(),
                        };
                        match result {
                            ReturnResult::Value(val) => val,
                            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                        }
                    },
                };
                
                let count = super::value::get_udata(value.to_value().unwrap()) as usize;

                partial_array.count = Some(count);
                count
            },
        };

        // Add already evaluated value.
        match old_result {
            Some(val) => partial_array.values.push(val),
            None => (),
        };

        // Get type attribute unit and die.
        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;


        // Evaluate all the values in the array.
        let start = partial_array.values.len();
        for _i in start..count {
            match self.eval_type(dwarf, &type_unit, type_die, data_offset, None, true)?.unwrap() { // TODO: Fix so that it can read multiple of the same type.
                ReturnResult::Value(val) => partial_array.values.push(val),
                ReturnResult::Required(req) => {
                    self.stack[current_state].partial_value = super::value::PartialValue::Array(partial_array);
                    return Ok(Some(ReturnResult::Required(req)));
                },
            };
        }
        
        self.stack.pop();
        Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Array(Box::new(super::value::ArrayValue {values: partial_array.values})))))
    }


    /*
     * Evaluate the value of a structure type.
     */
    pub fn eval_structured_type(&mut self,
                                dwarf:          &gimli::Dwarf<R>,
                                unit:           &gimli::Unit<R>,
                                die:            &gimli::DebuggingInformationEntry<'_, '_, R>,
                                data_offset:    u64,
                                old_result:     Option<EvaluatorValue<R>>,
                                new_state:      bool
                                ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has the tag DW_TAG_structure_type.
        match die.tag() {
            gimli::DW_TAG_structure_type => (),
            _ => panic!("Wrong implementation"),
        };

        // Get the index of the current state.
        let mut current_state = self.stack.len() - 1;

        // Create a new state if it doesn't already exist.
        if new_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
            current_state += 1;
        }

        self.check_alignment(die, data_offset)?;

        // Get the partial struct value from the current state.
        let mut partial_struct = match &self.stack[current_state].partial_value {
            super::value::PartialValue::Struct   (struct_) => struct_.clone(),
            e => panic!("{:?}", e),//return Err(anyhow!("Critical Error: expected partial struct")),
        };

        // Get all the DW_TAG_member dies.
        let children = get_children(unit, die);
        let mut member_dies = Vec::new();
        for c in &children {
            let c_die = unit.entry(*c)?;
            match c_die.tag() {
                // If it is a DW_TAG_variant_part die then it is a enum and only have on value.
                gimli::DW_TAG_variant_part => {

                    // Get the value.
                    let members = match old_result {
                        Some(val) => vec!(val),
                        None => {
                            match self.eval_variant_part(dwarf, unit, &c_die, data_offset, None, true)?.unwrap() {
                                ReturnResult::Value(val) => vec!(val),
                                ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                            }
                        },
                    };

                    self.stack.pop();
                    return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Struct(Box::new(super::value::StructValue {
                        name:       partial_struct.name,
                        members:    members,
                    })))));
                },
                gimli::DW_TAG_member => {
                    let data_member_location = attributes::data_member_location_attribute(&c_die).unwrap();
                    member_dies.push((data_member_location, c_die))
                },
                _ => continue,
            };
        }

        // Add already evaluated value.
        match old_result {
            Some(val) => partial_struct.members.push(val),
            None => (),
        };

        // Sort the members in the evaluation order.
        member_dies.sort_by_key(|m| m.0);

        // Evaluate all the members.
        let start = partial_struct.members.len();
        for i in start..member_dies.len() {
            let m_die = &member_dies[i].1;
            let member = match self.eval_member(dwarf, unit, m_die, data_offset, None, true)?.unwrap() {
                ReturnResult::Value(val) => val,
                ReturnResult::Required(req) => {
                    self.stack[current_state].partial_value = super::value::PartialValue::Struct(partial_struct);
                    return Ok(Some(ReturnResult::Required(req)));
                },
            };
            partial_struct.members.push(member);
        }

        self.stack.pop();
        return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Struct(Box::new(super::value::StructValue {
            name:       partial_struct.name,
            members:    partial_struct.members,
        })))));
    }


    /*
     * Evaluate the value of a union type.
     */
    pub fn eval_union_type(&mut self,
                           dwarf:       &gimli::Dwarf<R>,
                           unit:        &gimli::Unit<R>,
                           die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                           data_offset: u64,
                           old_result:  Option<EvaluatorValue<R>>,
                           new_state:   bool
                           ) -> Result<Option<ReturnResult<R>>>
    { 
        // Make sure that the die has the tag DW_TAG_union_type.
        match die.tag() {
            gimli::DW_TAG_union_type => (),
            _ => panic!("Wrong implementation"),
        };

        // Get the index of the current state.
        let mut current_state = self.stack.len() - 1;

        // Create a new state if it doesn't already exist.
        if new_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
            current_state += 1;
        } 

        self.check_alignment(die, data_offset)?;

        // Get the partial union value from the current state.
        let mut partial_union = match &self.stack[current_state].partial_value {
            super::value::PartialValue::Union   (union) => union.clone(),
            _ => return Err(anyhow!("Critical Error: expected partial union")),
        };

        // Get all children of type DW_TAG_member.
        let children = get_children(unit, die);
        let mut member_dies = vec!();
        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_member => {
                    let data_member_location = attributes::data_member_location_attribute(&c_die).unwrap();
                    member_dies.push((data_member_location, c_die))
                },
                _ => continue,
            };
        }

        // Add already evaluated value.
        match old_result {
            Some(val)   => partial_union.members.push(val),
            None        => (),
        };

        // Sort all the members in the order they need to be evaluated.
        member_dies.sort_by_key(|m| m.0);

        // Evaluate all the members.
        let start = partial_union.members.len();
        for i in start..member_dies.len() {
            let m_die = &member_dies[i].1;
            let member = match self.eval_member(dwarf, unit, m_die, data_offset, None, true)?.unwrap() {
                ReturnResult::Value(val) => val,
                ReturnResult::Required(req) => {
                    self.stack[current_state].partial_value = super::value::PartialValue::Union(partial_union);
                    return Ok(Some(ReturnResult::Required(req)));
                },
            };
            partial_union.members.push(member);
        }


        self.stack.pop();
        return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Union(Box::new(super::value::UnionValue {
            name:       partial_union.name,
            members:    partial_union.members,
        })))));
    }


    /*
     * Evaluate the value of a member.
     */
    pub fn eval_member(&mut self,
                       dwarf:           &gimli::Dwarf<R>,
                       unit:            &gimli::Unit<R>,
                       die:             &gimli::DebuggingInformationEntry<'_, '_, R>,
                       data_offset:     u64,
                       old_result:  Option<EvaluatorValue<R>>,
                       create_state:    bool
                       ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has the tag DW_TAG_member
        match die.tag() {
            gimli::DW_TAG_member => (),
            _ => panic!("Wrong implementation"),
        };

        // Create a new state if it doesn't already exist.
        if create_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
        }

        // Get the name of the member.
        let name = attributes::name_attribute(dwarf, die);

        // If value is already evaluated, then use it.
        match old_result {
            Some(val) => {
                self.stack.pop();
                return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Member(Box::new(super::value::MemberValue {
                    name:   name,
                    value:  val,
                })))));
            },
            None => (),
        };

        // Calculate the new data offset.
        let new_data_offset = match attributes::data_member_location_attribute(die) { // NOTE: Seams it can also be a location description and not an offset. Dwarf 5 page 118
            Some(val)   => data_offset + val,
            None        => data_offset,
        };
        
        self.check_alignment(die, new_data_offset)?;

        // Get the type attribute unit and die.
        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;

        // Evaluate the value.
        let value = match self.eval_type(dwarf, &type_unit, type_die, new_data_offset, old_result, true)?.unwrap() {
            ReturnResult::Value(val) => val,
            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
        };

        self.stack.pop();
        Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Member(Box::new(super::value::MemberValue {
            name:   name,
            value:  value
        })))))
    }


    /*
     * Evaluate the value of a enumeration type.
     */
    pub fn eval_enumeration_type(&mut self,
                                 dwarf:         &gimli::Dwarf<R>,
                                 unit:          &gimli::Unit<R>,
                                 die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                                 data_offset:   u64,
                                 old_result: Option<EvaluatorValue<R>>,
                                 new_state:     bool,
                                 ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has the tag DW_TAG_enumeration_type
        match die.tag() {
            gimli::DW_TAG_enumeration_type => (),
            _ => panic!("Wrong implementation"),
        };

        // Create a new state if it doesn't already exist.
        if new_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
        }

        self.check_alignment(die, data_offset)?;

        // Get type attribute unit and die.
        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;

        // Get type value.
        let type_result = match old_result {
            // Use already evaluated value.
            Some(val)   => val,
            // Evaluate the type value.
            None        => {
                match self.eval_type(dwarf, &type_unit, type_die, data_offset, old_result, true)?.unwrap() {
                    ReturnResult::Value(val) => val,
                    ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                }
            },
        };

        // Get the value as a unsigned int.
        let value = super::value::get_udata(type_result.to_value().unwrap());

        // Go through the children and find the correct enumerator value.
        let children = get_children(unit, die);
        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_enumerator  => {
                    let const_value = attributes::const_value_attribute(&c_die).unwrap();

                    // Check if it is the correct one.
                    if const_value == value {

                        // Get the name of the enum type and the enum variant.
                        let name = attributes::name_attribute(dwarf, die).unwrap(); 
                        let e_name = attributes::name_attribute(dwarf, &c_die).unwrap(); 

                        self.stack.pop();
                        return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::EnumValue {
                            name:   name,
                            value:  super::value::EvaluatorValue::Name(e_name),
                        })))));
                    }
                },
                gimli::DW_TAG_subprogram => (),
                _ => unimplemented!(),
            };
        }

        self.stack.pop();
        Ok(None)
    }


    /*
     * Evaluate the value of a subrange type.
     */
    pub fn eval_subrange_type(&mut self,
                              dwarf:        &gimli::Dwarf<R>,
                              unit:          &gimli::Unit<R>,
                              die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                              data_offset:   u64,
                              old_result:   Option<EvaluatorValue<R>>,
                              ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has the tag DW_TAG_subrange_type
        match die.tag() {
            gimli::DW_TAG_subrange_type => (),
            _ => panic!("Wrong implementation"),
        };

        // If the die has a count attribute then that is the value.
        match attributes::count_attribute(die) { // NOTE: This could be replace with lower and upper bound
            Some(val)   => return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Value(BaseValue::U64(val))))),
            None        => (),
        };

        // Use the already evaluated value.
        match old_result {
            Some(val)   => return Ok(Some(ReturnResult::Value(val))),
            None        => (),
        };

        // Get the type unit and die.
        let (type_unit, die_offset) = match self.get_type_info(dwarf, unit, die) {
            Ok(val) => val,
            Err(_) => return Ok(None),
        };
        let type_die = &type_unit.entry(die_offset)?;

        // Evaluate the type attribute value.
        self.eval_type(dwarf, &type_unit, type_die, data_offset, old_result, true)
    }


    /*
     * Evaluate the value of a variant part.
     */
    pub fn eval_variant_part(&mut self,
                             dwarf:         &gimli::Dwarf<R>,
                             unit:          &gimli::Unit<R>,
                             die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                             data_offset:   u64,
                             mut old_result: Option<EvaluatorValue<R>>,
                             new_state:     bool
                             ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die has tag DW_TAG_variant_part
        match die.tag() {
            gimli::DW_TAG_variant_part => (),
            _ => panic!("Wrong implementation"),
        };

        // Get the current state index.
        let mut current_state = self.stack.len() - 1;

        // Create a new state if it doesn't already exist.
        if new_state {
            self.stack.push(EvaluatorState::new(dwarf, unit, die, data_offset));
            current_state += 1;
        }

        self.check_alignment(die, data_offset)?;

        // Get the partial value of the current state.
        let mut partial_variant_part = match &self.stack[current_state].partial_value {
            super::value::PartialValue::VariantPart   (vp) => vp.clone(),
            _ => return Err(anyhow!("Critical Error: expected partial variant_part")),
        };


        // Get the enum variant.
        // TODO: If variant is optimised out then return optimised out and remove the pieces for
        // this type if needed.
        let variant = match partial_variant_part.variant {
            Some(val) => val, // Use the value stored in the state.
            None      => {
                let value = match old_result {
                    Some(val)   => {
                        // Use the already evaluated value.
                        old_result = None;
                        val
                    },
                    None        => {
                        // Get member die.
                        let die_offset = attributes::discr_attribute(die).unwrap();
                        let member = &unit.entry(die_offset).unwrap();

                        // Evaluate the DW_TAG_member value.
                        match self.eval_member(dwarf, unit, member, data_offset, None, true)?.unwrap() {
                            ReturnResult::Value(val) => val,
                            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                        }
                    },
                };

                // The value should be a unsigned int thus convert the value to a u64.
                let variant = super::value::get_udata(value.to_value().unwrap());
                partial_variant_part.variant = Some(variant); // Store variant value in state.
                variant
            },
        };


        // Find the DW_TAG_member die and all the DW_TAG_variant dies.
        let mut variants = vec!();
        let children = get_children(unit, die);
        for c in &children {
            let c_die = unit.entry(*c)?;
            match c_die.tag() {
                gimli::DW_TAG_variant => {
                    variants.push(c_die);
                },
                _ => (),
            };
        }

        for v in &variants {
            // Find the right variant type and evaluate it.
            let discr_value = attributes::discr_value_attribute(v).unwrap();

            // Check if it is the correct variant.
            if discr_value == variant % (variants.len() as u64) { // NOTE: Don't know if using modulus here is correct, but it seems to be correct.

                // Evaluate the value of the variant.
                match self.eval_variant(dwarf, unit, v, data_offset, old_result)?.unwrap() {
                    ReturnResult::Value(val) => {
                        self.stack.pop();
                        return Ok(Some(ReturnResult::Value(val)));
                    },
                    ReturnResult::Required(req) =>{
                        self.stack[current_state].partial_value = super::value::PartialValue::VariantPart(partial_variant_part);
                        return Ok(Some(ReturnResult::Required(req)));
                    }, 
                };
            }
        }
    
        panic!("Should never reach here");
    }


    /*
     * Evaluate the value of a variant.
     */
    pub fn eval_variant(&mut self,
                        dwarf:          &gimli::Dwarf<R>,
                        unit:           &gimli::Unit<R>,
                        die:            &gimli::DebuggingInformationEntry<'_, '_, R>,
                        data_offset:    u64,
                        old_result:     Option<EvaluatorValue<R>>,
                        ) -> Result<Option<ReturnResult<R>>>
    {
        // Make sure that the die is of type DW_TAG_variant
        match die.tag() {
            gimli::DW_TAG_variant => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;

        // Find the child die of type DW_TAG_member
        let children = get_children(unit, die);
        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_member => {

                    // Get the value of the member.
                    let value = match old_result {
                        Some(val)   => val, // Use the already evaluated value.
                        None        => {
                            // Evaluate the value of the member.
                            match self.eval_member(dwarf, unit, &c_die, data_offset, None, true)?.unwrap() {
                                ReturnResult::Value(val) => val,
                                ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                            }
                        },
                    };

                    // Get the name of the die.
                    let name = attributes::name_attribute(dwarf, &c_die).unwrap();

                    return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::EnumValue {
                        name:   name,
                        value:  value,
                    })))));
                },
                _ => (),
            };
        }
        unimplemented!();
    }

    /*
     * Check if address is correctly aligned
     *
     * NOTE: Don't know if it is correct.
     */
    fn check_alignment(&mut self,
                       die:             &gimli::DebuggingInformationEntry<'_, '_, R>,
                       mut data_offset: u64,
                       ) -> Result<()>
    {
        match attributes::alignment_attribute(die) {
            Some(alignment) => {
                if self.pieces.len() <= self.piece_index {
                    return Ok(());
                }
                
                if self.pieces.len() < 1 {
                    data_offset = 0;
                }
                
                match self.pieces[self.piece_index].location {
                    Location::Address { address } => {
                        let mut addr = address + (data_offset/4) * 4;
                        addr -= addr%4; // TODO: Is this correct?

                        if addr % alignment != 0 {
                            panic!("address not aligned");
                            return Err(anyhow!("Address not aligned"));
                        }
                    },
                    _ => (),
                };

            },
            None => (),
        };

        Ok(())
    }
}


/*
 * Helper function for getting all the children of a die.
 */
fn get_children<R: Reader<Offset = usize>>(unit: &gimli::Unit<R>,
                                           die: &gimli::DebuggingInformationEntry<'_, '_, R>
                                           ) -> Vec<gimli::UnitOffset>
{
    let mut result = Vec::new();
    let mut tree = unit.entries_tree(Some(die.offset())).unwrap();
    let node = tree.root().unwrap();

    let mut children = node.children();
    while let Some(child) = children.next().unwrap() { 
        result.push(child.entry().offset());
    }
    
    result
}


/*
 * Evaluates the value of a base type.
 */
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

        (DwAte(2), 1) => BaseValue::Bool((value as u8) == 1), // Should be returned as bool?
        (DwAte(1), 4) => BaseValue::Address32(value as u32),
        _ => {
            println!("{:?}, {:?}", encoding, byte_size);
            unimplemented!()
        },
    }
}


/*
 * Helper function that turns slice into a u64
 */
pub fn slize_as_u64(data: &[u32]) -> u64
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


/*
 * Evaluates the value of a base type.
 */
pub fn parse_base_type<R>(unit:         &gimli::Unit<R>,
                      data:         &[u32],
                      base_type:    gimli::UnitOffset<usize>
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
        Ok(Some(gimli::AttributeValue::Encoding(dwate))) => dwate,
        _ => panic!("expected Encoding"),
    };
    let byte_size = match die.attr_value(gimli::DW_AT_byte_size) {
        Ok(Some(gimli::AttributeValue::Udata(v))) => v,
        _ => panic!("expected Udata"),
    };
    
    eval_base_type(data, encoding, byte_size)
}

