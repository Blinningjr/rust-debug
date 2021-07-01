use super::{
    attributes,
    value::{
        BaseValue,
        EvaluatorValue,
    },
};
use std::convert::TryInto;

use crate::rust_debug::MemoryAndRegisters;

use crate::rust_debug::evaluate::value_information::ValueInformation;
use crate::rust_debug::evaluate::value_information::ValuePiece;

use gimli::{
    Reader,
    Piece,
    Location,
    DwAte,
};


use anyhow::{
    bail,
    Result,
};


/*
 * The state of a partially evaluated type.
 */
#[derive(Debug)]
struct EvaluatorState {
    pub unit_offset:    gimli::UnitSectionOffset,
    pub die_offset:     gimli::UnitOffset,
    pub data_offset:    u64,
}


impl EvaluatorState {
    pub fn new<R: Reader<Offset = usize>>(unit:        &gimli::Unit<R>,
                                          die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                                          data_offset: u64
                                          ) -> EvaluatorState
    {
        EvaluatorState {
            unit_offset:    unit.header.offset(),
            die_offset:     die.offset(),
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


pub enum PieceResult<R: Reader<Offset = usize>> {
    Value(Vec<u8>, Vec<ValuePiece>),
    Bytes(R),
    OptimizedOut,
    Required(EvaluatorResult),
}


/*
 * Evaluates the value of a type given Dwarf pieces.
 */
pub struct Evaluator<R: Reader<Offset = usize>> {
    pieces:         Vec<Piece<R>>,
    piece_index:    usize,
    stack:          Option<EvaluatorState>,
    result:         Option<super::value::EvaluatorValue<R>>,
}


impl<R: Reader<Offset = usize>> Evaluator<R> {
    pub fn new(pieces:  Vec<Piece<R>>,
               unit:    Option<&gimli::Unit<R>>,
               die:     Option<&gimli::DebuggingInformationEntry<'_, '_, R>>
               ) -> Evaluator<R>
    {
        // If no unit and die is given then the first piece will be evaluated.
        let stack = match unit {
            Some(u) => {
                match die {
                    Some(d) => Some(EvaluatorState::new(u, d, 0)),
                    None => None,
                }
            },
            None => None,
        };
        Evaluator {
            pieces:         pieces,
            piece_index:    0,
            stack:          stack,
            result:         None,
        }
    }


    pub fn evaluate(&mut self, dwarf: &gimli::Dwarf<R>, memory_and_registers: &MemoryAndRegisters) -> Result<EvaluatorResult> {
        self.piece_index = 0;

        // If the value has already been evaluated then don't evaluated it again.
        if self.result.is_some() {
            return Ok(EvaluatorResult::Complete);
        }

        // Check if a type die was given and if it was then get the needed information.
        // Otherwise just evaluate the first piece into a u32.
        let (unit_offset, die_offset, data_offset) = match &self.stack {
            Some(state) => {
                // Get the current state information.
                (state.unit_offset, state.die_offset, state.data_offset)
            },
            None => {
                // If the stack is empty then the first piece will be evaluated.
                let result = self.handle_eval_piece(memory_and_registers, Some(4), 0, Some(DwAte(1)))?;
                match result {
                    ReturnResult::Value(val) => {
                        self.result = Some(val);
                        return Ok(EvaluatorResult::Complete);
                    },
                    ReturnResult::Required(req) => return Ok(req),
                };
            },
        };

        
        // Get the unit of the current state.
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
                match result {
                    Some(val) => val,
                    None => bail!("Could not find unit form offset"),
                }
            },
        };

        // Get the die of the current state.
        let die = &unit.entry(die_offset)?;

        // Continue evaluating the value of the current state.
        match self.eval_type(memory_and_registers, dwarf, &unit, die, data_offset)? {
        ReturnResult::Value(val) => {
                self.result = Some(val);
                Ok(EvaluatorResult::Complete)
            },
            ReturnResult::Required(req) => Ok(req),
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
        let (unit_offset, die_offset) = match attributes::type_attribute(dwarf, unit, die)? {
            Some(val) => val,
            None => bail!("Die dosen't have the required DW_AT_type attribute"),
        };
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
                match result {
                    Some(val) => val,
                    None => bail!("Could not get unit from unit offset"),
                }
            },
        };
       
        Ok((unit, die_offset))
    }

    /*
     * Evaluates the value of a type.
     */
    pub fn eval_type(&mut self,
                     memory_and_registers: &MemoryAndRegisters,
                     dwarf:         &gimli::Dwarf<R>,
                     unit:          &gimli::Unit<R>,
                     die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                     data_offset:   u64,
                     ) -> Result<ReturnResult<R>>
    { 
        match die.tag() {
            gimli::DW_TAG_base_type                 => self.eval_basetype(memory_and_registers, die, data_offset),
            gimli::DW_TAG_pointer_type              => self.eval_pointer_type(memory_and_registers, die, data_offset),
            gimli::DW_TAG_array_type                => self.eval_array_type(memory_and_registers, dwarf, unit, die, data_offset),
            gimli::DW_TAG_structure_type            => self.eval_structured_type(memory_and_registers, dwarf, unit, die, data_offset),
            gimli::DW_TAG_union_type                => self.eval_union_type(memory_and_registers, dwarf, unit, die, data_offset),
            gimli::DW_TAG_member                    => self.eval_member(memory_and_registers, dwarf, unit, die, data_offset),
            gimli::DW_TAG_enumeration_type          => self.eval_enumeration_type(memory_and_registers, dwarf, unit, die, data_offset),
            gimli::DW_TAG_string_type               => unimplemented!(),
            gimli::DW_TAG_generic_subrange          => unimplemented!(),
            gimli::DW_TAG_template_type_parameter   => unimplemented!(),
            gimli::DW_TAG_variant_part              => self.eval_variant_part(memory_and_registers, dwarf, unit, die, data_offset),
            gimli::DW_TAG_subroutine_type           => unimplemented!(),
            gimli::DW_TAG_subprogram                => unimplemented!(),
            _ => unimplemented!(),
        }
    }


    /*
     * Evaluate the value of a piece.
     */
    pub fn eval_piece(&mut self,
                      memory_and_registers: &MemoryAndRegisters,
                      piece:        Piece<R>,
                      byte_size:    u64,
                      data_offset:  u64,
                      ) -> PieceResult<R>
    {
        match piece.location {
            Location::Empty                                         => PieceResult::OptimizedOut,
            Location::Register        { ref register }              => self.eval_register(memory_and_registers, register, &piece),
            Location::Address         { address }                   => self.eval_address(memory_and_registers, address, byte_size, data_offset, &piece),
            Location::Value           { value }                     => self.eval_gimli_value(value, &piece),
            Location::Bytes           { value }                     => PieceResult::Bytes(value.clone()),
            Location::ImplicitPointer { value: _, byte_offset: _ }  => unimplemented!(),
        }
    }


    pub fn eval_gimli_value(&mut self,
                         value:     gimli::Value,
                         piece:     &Piece<R>,
                         ) -> PieceResult<R>
    {
        let mut bytes = vec!();
        match value {
            gimli::Value::Generic(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::I8(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::U8(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::I16(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::U16(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::I32(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::U32(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::I64(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::U64(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::F32(val) => bytes.extend_from_slice(&val.to_le_bytes()),
            gimli::Value::F64(val) => bytes.extend_from_slice(&val.to_le_bytes()),
        };

        bytes = trim_piece_bytes(bytes, piece, 4);
        let byte_size = bytes.len();

        return PieceResult::Value(bytes, vec!(ValuePiece::Dwarf { byte_size: byte_size }));
    }


    /*
     * Evaluate the value of a register.
     */
    pub fn eval_register(&mut self,
                         memory_and_registers: &MemoryAndRegisters,
                         register:  &gimli::Register,
                         piece:     &Piece<R>,
                         ) -> PieceResult<R>
    {
        match memory_and_registers.get_register_value(&register.0) {
            Some(val) => { // TODO: Mask the important bits?
                let mut bytes = vec!();
                bytes.extend_from_slice(&val.to_le_bytes());
                
                bytes = trim_piece_bytes(bytes, piece, 4);
                let byte_size = bytes.len();

                return PieceResult::Value(bytes, vec!(ValuePiece::Register { register: register.0, byte_size: byte_size }));
            },
            None    => PieceResult::Required(
                    EvaluatorResult::RequireReg(register.0)),
        }
    }


    /*
     * Evaluate the value of a address.
     */
    pub fn eval_address(&mut self,
                        memory_and_registers: &MemoryAndRegisters,
                        mut address:    u64,
                        byte_size:      u64,
                        data_offset:    u64,
                        piece:     &Piece<R>,
                        ) -> PieceResult<R>
    {
        address += data_offset;

        let num_bytes = match piece.size_in_bits {
            Some(val) => (val + 8 - 1)/8,
            None => byte_size,
        } as usize;

        let bytes = match memory_and_registers.get_addresses(&(address as u32), num_bytes) {
            Some(val) => val,
            None => return PieceResult::Required(EvaluatorResult::RequireData {
                address: address as u32,
                num_words: num_bytes,
            }),
        };

        PieceResult::Value(bytes, vec!(ValuePiece::Memory { address: address as u32, byte_size: num_bytes }))    
    }


    /*
     * Evaluates the value of a piece and decides if the piece should be discarded or kept.
     */
    pub fn handle_eval_piece(&mut self,
                             memory_and_registers: &MemoryAndRegisters,
                             byte_size:         Option<u64>,
                             mut data_offset:   u64,
                             encoding:          Option<DwAte>
                             ) -> Result<ReturnResult<R>>
    {
        if self.pieces.len() <= self.piece_index {
            return Ok(ReturnResult::Value(super::value::EvaluatorValue::OptimizedOut));
        }

        // TODO: confirm
        if self.pieces.len() > 1 { // NOTE: Is this correct?
            data_offset = 0;
        }

        let num_bytes = match byte_size {
            Some(val) => val,
            None => bail!("Requires byte size"),
        };

        let encode = match encoding {
            Some(val) => val,
            None => bail!("Requires encoding"),
        };

        let result = self.get_bytes(memory_and_registers, num_bytes, data_offset)?;
        return match result {
            PieceResult::Value(bytes, value_pieces) => Ok(ReturnResult::Value(super::value::EvaluatorValue::Value(new_eval_base_type(bytes.clone(), encode), ValueInformation::new(Some(bytes.clone()), value_pieces)))),
            PieceResult::Bytes(bytes) => Ok(ReturnResult::Value(super::value::EvaluatorValue::Bytes(bytes))),
            PieceResult::OptimizedOut => Ok(ReturnResult::Value(super::value::EvaluatorValue::OptimizedOut)),
            PieceResult::Required(required) => Ok(ReturnResult::Required(required)),
        }
    }


    fn get_bytes(&mut self,
                 memory_and_registers:  &MemoryAndRegisters,
                 byte_size:             u64,
                 mut data_offset:       u64,
                 ) -> Result<PieceResult<R>>
    {
        // TODO: confirm
        if self.pieces.len() > 1 { // NOTE: Is this correct?
            data_offset = 0;
        }

        let mut bytes = vec!();
        let mut value_pieces = vec!();
        while  bytes.len() < byte_size.try_into()? {

            if self.pieces.len() <= self.piece_index {
                unimplemented!();
                //return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::OptimizedOut)));
            }
            let piece = self.pieces[self.piece_index].clone();
            let result = self.eval_piece(memory_and_registers,
                                         piece,
                                         byte_size,
                                         data_offset);
            let (new_bytes, value_piece) = match result {
                PieceResult::Value(bytes, pieces) => (bytes, pieces),
                _ => return Ok(result),
            };

            bytes.extend_from_slice(&new_bytes);
            value_pieces.extend_from_slice(&value_piece);
            if self.pieces[self.piece_index].size_in_bits.is_some() {
                self.piece_index += 1;
            }
        }

//        while bytes.len() > byte_size as usize {
//            bytes.pop();    // TODO: Think this loop can be removed
//        }
        
        return Ok(PieceResult::Value(bytes, value_pieces));
    }




    /*
     * Evaluate the value of a base type.
     */
    pub fn eval_basetype(&mut self,
                         memory_and_registers: &MemoryAndRegisters,
                         die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                         data_offset:   u64,
                         ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has the tag DW_TAG_base_type.
        match die.tag() {
            gimli::DW_TAG_base_type => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;

        // Get byte size and encoding from the die.
        let byte_size = attributes::byte_size_attribute(die);
        let encoding =  attributes::encoding_attribute(die);
        match byte_size {
            // If the byte size is 0 then the value is optimized out.
            Some(0) => {
                return Ok(ReturnResult::Value(super::value::EvaluatorValue::ZeroSize));
            },
            _       => (),
        };

        // Evaluate the value.
        match self.handle_eval_piece(memory_and_registers,
                                     byte_size,
                                     data_offset, // TODO
                                     encoding)?
        {
            ReturnResult::Value(val) => {
                return Ok(ReturnResult::Value(val));
            },
            ReturnResult::Required(req) => return Ok(ReturnResult::Required(req)),
        };
    }


    /*
     * Evaluate the value of a pointer type.
     */
    pub fn eval_pointer_type(&mut self,
                             memory_and_registers: &MemoryAndRegisters,
                             die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                             data_offset:   u64,
                             ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has the tag DW_TAG_array_type.
        match die.tag() {
            gimli::DW_TAG_pointer_type => (),
            _ => panic!("Wrong implementation"),
        };        
 
        self.check_alignment(die, data_offset)?;

        // Evaluate the pointer type value.
        let address_class = match attributes::address_class_attribute(die) {
            Some(val) => val,
            None => bail!("Die is missing required attribute DW_AT_address_class"),
        };
        match address_class.0 {
            0 => {
                let res = self.handle_eval_piece(memory_and_registers,
                                                 Some(4),
                                                 data_offset,
                                                 Some(DwAte(1)))?;
                return Ok(res);
            },
            _ => panic!("Unimplemented DwAddr code"), // NOTE: The codes are architecture specific.
        };
    }


    /*
     * Evaluate the value of a array type.
     */
    pub fn eval_array_type(&mut self,
                           memory_and_registers: &MemoryAndRegisters,
                           dwarf:       &gimli::Dwarf<R>,
                           unit:        &gimli::Unit<R>,
                           die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                           data_offset: u64,
                           ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has the tag DW_TAG_array_type.
        match die.tag() {
            gimli::DW_TAG_array_type => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;

        let children = get_children(unit, die)?;
        let dimension_die = unit.entry(children[0])?;

        let result = match dimension_die.tag() {
            gimli::DW_TAG_subrange_type     => self.eval_subrange_type(memory_and_registers, dwarf, unit, &dimension_die, data_offset)?,
            gimli::DW_TAG_enumeration_type  => self.eval_enumeration_type(memory_and_registers, dwarf, unit, &dimension_die, data_offset)?,
            _ => unimplemented!(),
        };

        let value = match result {
            ReturnResult::Value(val) => val,
            ReturnResult::Required(req) => return Ok(ReturnResult::Required(req)),
        };
        
        // Evaluate the length of the array.
        let count = super::value::get_udata(match value.to_value() {
            Some(val) => val,
            None => return Ok(ReturnResult::Value(EvaluatorValue::OptimizedOut)), // TODO: Maybe need to remove the following pieces that is related to this structure.
        }) as usize;


        // Get type attribute unit and die.
        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;


        // Evaluate all the values in the array.
        let mut values = vec!();
        for _i in 0..count {
            match self.eval_type(memory_and_registers, dwarf, &type_unit, type_die, data_offset)? { // TODO: Fix so that it can read multiple of the same type.
                ReturnResult::Value(val) => values.push(val),
                ReturnResult::Required(req) => {
                    return Ok(ReturnResult::Required(req));
                },
            };
        }
        
        Ok(ReturnResult::Value(super::value::EvaluatorValue::Array(Box::new(super::value::ArrayValue {values: values}))))
    }


    /*
     * Evaluate the value of a structure type.
     */
    pub fn eval_structured_type(&mut self,
                                memory_and_registers: &MemoryAndRegisters,
                                dwarf:          &gimli::Dwarf<R>,
                                unit:           &gimli::Unit<R>,
                                die:            &gimli::DebuggingInformationEntry<'_, '_, R>,
                                data_offset:    u64,
                                ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has the tag DW_TAG_structure_type.
        match die.tag() {
            gimli::DW_TAG_structure_type => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;


        let name = match attributes::name_attribute(dwarf, die) {
            Some(val) => val,
            None => bail!("Expected the structure type die to have a name attribute"),
        };

        // Get all the DW_TAG_member dies.
        let children = get_children(unit, die)?;
        let mut member_dies = Vec::new();
        for c in &children {
            let c_die = unit.entry(*c)?;
            match c_die.tag() {
                // If it is a DW_TAG_variant_part die then it is a enum and only have on value.
                gimli::DW_TAG_variant_part => {

                    // Get the value.
                    let members = match self.eval_variant_part(memory_and_registers, dwarf, unit, &c_die, data_offset)? {
                        ReturnResult::Value(val) => vec!(val),
                        ReturnResult::Required(req) => return Ok(ReturnResult::Required(req)),
                    };


                    return Ok(ReturnResult::Value(super::value::EvaluatorValue::Struct(Box::new(super::value::StructValue {
                        name:       name,
                        members:    members,
                    }))));
                },
                gimli::DW_TAG_member => {
                    let data_member_location = match attributes::data_member_location_attribute(&c_die) {
                        Some(val) => val,
                        None => bail!("Expacted member die to have attribute DW_AT_data_member_location"),
                    };
                    member_dies.push((data_member_location, c_die))
                },
                _ => continue,
            };
        }


        // Sort the members in the evaluation order.
        member_dies.sort_by_key(|m| m.0);


        // Evaluate all the members.
        let mut members = vec!();
        for i in 0..member_dies.len() {
            let m_die = &member_dies[i].1;
            let member = match self.eval_member(memory_and_registers, dwarf, unit, m_die, data_offset)? {
                ReturnResult::Value(val) => val,
                ReturnResult::Required(req) => {
                    return Ok(ReturnResult::Required(req));
                },
            };
            members.push(member);
        }


        return Ok(ReturnResult::Value(super::value::EvaluatorValue::Struct(Box::new(super::value::StructValue {
            name:       name,
            members:    members,
        }))));
    }


    /*
     * Evaluate the value of a union type.
     */
    pub fn eval_union_type(&mut self,
                           memory_and_registers: &MemoryAndRegisters,
                           dwarf:       &gimli::Dwarf<R>,
                           unit:        &gimli::Unit<R>,
                           die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                           data_offset: u64,
                           ) -> Result<ReturnResult<R>>
    { 
        // Make sure that the die has the tag DW_TAG_union_type.
        match die.tag() {
            gimli::DW_TAG_union_type => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;


        let name = match attributes::name_attribute(dwarf, die) {
            Some(val) => val,
            None => bail!("Expected untion type die to have a name attribute"),
        };

        // Get all children of type DW_TAG_member.
        let children = get_children(unit, die)?;
        let mut member_dies = vec!();
        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_member => {
                    let data_member_location = match attributes::data_member_location_attribute(&c_die) {
                        Some(val) => val,
                        None => bail!("Expected member die to have attribute DW_AT_data_member_location"),
                    };
                    member_dies.push((data_member_location, c_die))
                },
                _ => continue,
            };
        }

        // Sort all the members in the order they need to be evaluated.
        member_dies.sort_by_key(|m| m.0);


        // Evaluate all the members.
        let mut members = vec!();
        for i in 0..member_dies.len() {
            let m_die = &member_dies[i].1;
            let member = match self.eval_member(memory_and_registers, dwarf, unit, m_die, data_offset)? {
                ReturnResult::Value(val) => val,
                ReturnResult::Required(req) => {
                    return Ok(ReturnResult::Required(req));
                },
            };
            members.push(member);
        }


        return Ok(ReturnResult::Value(super::value::EvaluatorValue::Union(Box::new(super::value::UnionValue {
            name:       name,
            members:    members,
        }))));
    }


    /*
     * Evaluate the value of a member.
     */
    pub fn eval_member(&mut self,
                       memory_and_registers: &MemoryAndRegisters,
                       dwarf:           &gimli::Dwarf<R>,
                       unit:            &gimli::Unit<R>,
                       die:             &gimli::DebuggingInformationEntry<'_, '_, R>,
                       data_offset:     u64,
                       ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has the tag DW_TAG_member
        match die.tag() {
            gimli::DW_TAG_member => (),
            _ => panic!("Wrong implementation"),
        };

        // Get the name of the member.
        let name = attributes::name_attribute(dwarf, die);

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
        let value = match self.eval_type(memory_and_registers, dwarf, &type_unit, type_die, new_data_offset)? {
            ReturnResult::Value(val) => val,
            ReturnResult::Required(req) => return Ok(ReturnResult::Required(req)),
        };

        Ok(ReturnResult::Value(super::value::EvaluatorValue::Member(Box::new(super::value::MemberValue {
            name:   name,
            value:  value
        }))))
    }


    /*
     * Evaluate the value of a enumeration type.
     */
    pub fn eval_enumeration_type(&mut self,
                                 memory_and_registers: &MemoryAndRegisters,
                                 dwarf:         &gimli::Dwarf<R>,
                                 unit:          &gimli::Unit<R>,
                                 die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                                 data_offset:   u64,
                                 ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has the tag DW_TAG_enumeration_type
        match die.tag() {
            gimli::DW_TAG_enumeration_type => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;

        // Get type attribute unit and die.
        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;

        // Get type value.
        let type_result = match self.eval_type(memory_and_registers, dwarf, &type_unit, type_die, data_offset)? {
            ReturnResult::Value(val) => val,
            ReturnResult::Required(req) => return Ok(ReturnResult::Required(req)),
        };

        // Get the value as a unsigned int.
        let value = super::value::get_udata(match type_result.to_value() {
            Some(val) => val,
            None => return Ok(ReturnResult::Value(EvaluatorValue::OptimizedOut)), // TODO: Maybe need to remove the following pieces that is related to this structure.
        });

        // Go through the children and find the correct enumerator value.
        let children = get_children(unit, die)?;

        let clen = children.len() as u64;

        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_enumerator  => {
                    let const_value = match attributes::const_value_attribute(&c_die) {
                        Some(val) => val,
                        None => bail!("Expected enumeration type die to have attribute DW_AT_const_value"),
                    };

                    // Check if it is the correct one.
                    if const_value == value % clen {

                        // Get the name of the enum type and the enum variant.
                        let name = match attributes::name_attribute(dwarf, die) {
                            Some(val) => val,
                            None => bail!("Expeceted enumeration type die to have attribute DW_AT_name"),
                        };

                        let e_name = match attributes::name_attribute(dwarf, &c_die) {
                            Some(val) => val,
                            None => bail!("Expeceted enumerator die to have attribute DW_AT_name"),
                        };

                        return Ok(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::EnumValue {
                            name:   name,
                            value:  super::value::EvaluatorValue::Name(e_name),
                        }))));
                    }
                },
                gimli::DW_TAG_subprogram => (),
                _ => unimplemented!(),
            };
        }

        unreachable!()
    }


    /*
     * Evaluate the value of a subrange type.
     */
    pub fn eval_subrange_type(&mut self,
                              memory_and_registers: &MemoryAndRegisters,
                              dwarf:        &gimli::Dwarf<R>,
                              unit:          &gimli::Unit<R>,
                              die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                              data_offset:   u64,
                              ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has the tag DW_TAG_subrange_type
        match die.tag() {
            gimli::DW_TAG_subrange_type => (),
            _ => panic!("Wrong implementation"),
        };

        // If the die has a count attribute then that is the value.
        match attributes::count_attribute(die) { // NOTE: This could be replace with lower and upper bound
            Some(val)   => return Ok(ReturnResult::Value(super::value::EvaluatorValue::Value(BaseValue::U64(val), ValueInformation::new(None, vec!(ValuePiece::Dwarf { byte_size: 0 }))))),
            None        => (),
        };

        // Get the type unit and die.
        let (type_unit, die_offset) = match self.get_type_info(dwarf, unit, die) {
            Ok(val) => val,
            Err(_) => bail!("Expected subrange type die to have type information"),
        };
        let type_die = &type_unit.entry(die_offset)?;

        // Evaluate the type attribute value.
        Ok(self.eval_type(memory_and_registers, dwarf, &type_unit, type_die, data_offset)?)
    }


    /*
     * Evaluate the value of a variant part.
     */
    pub fn eval_variant_part(&mut self,
                             memory_and_registers: &MemoryAndRegisters,
                             dwarf:         &gimli::Dwarf<R>,
                             unit:          &gimli::Unit<R>,
                             die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                             data_offset:   u64,
                             ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die has tag DW_TAG_variant_part
        match die.tag() {
            gimli::DW_TAG_variant_part => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;


        // Get the enum variant.
        // TODO: If variant is optimised out then return optimised out and remove the pieces for
        // this type if needed.

        // Get member die.
        let die_offset = match attributes::discr_attribute(die) {
            Some(val) => val,
            None => bail!("Expected variant part die to have attribute DW_AT_discr"),
        };
        let member = &unit.entry(die_offset)?;

        // Evaluate the DW_TAG_member value.
        let value = match self.eval_member(memory_and_registers, dwarf, unit, member, data_offset)? {
            ReturnResult::Value(val) => val,
            ReturnResult::Required(req) => return Ok(ReturnResult::Required(req)),
        };

        // The value should be a unsigned int thus convert the value to a u64.
        let variant = super::value::get_udata(match value.to_value() {
            Some(val) => val,
            None => return Ok(ReturnResult::Value(EvaluatorValue::OptimizedOut)), // TODO: Maybe need to remove the following pieces that is related to this structure.
        });


        // Find the DW_TAG_member die and all the DW_TAG_variant dies.
        let mut variants = vec!();
        let children = get_children(unit, die)?;
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
            let discr_value = match attributes::discr_value_attribute(v) {
                Some(val) => val,
                None => bail!("Expected variant die to have attribute DW_AT_discr_value"),
            };

            // Check if it is the correct variant.
            if discr_value == variant % (variants.len() as u64) { // NOTE: Don't know if using modulus here is correct, but it seems to be correct.

                // Evaluate the value of the variant.
                match self.eval_variant(memory_and_registers, dwarf, unit, v, data_offset)? {
                    ReturnResult::Value(val) => {
                        return Ok(ReturnResult::Value(val));
                    },
                    ReturnResult::Required(req) =>{
                        return Ok(ReturnResult::Required(req));
                    }, 
                };
            }
        }
    
        unreachable!();
    }


    /*
     * Evaluate the value of a variant.
     */
    pub fn eval_variant(&mut self,
                        memory_and_registers: &MemoryAndRegisters,
                        dwarf:          &gimli::Dwarf<R>,
                        unit:           &gimli::Unit<R>,
                        die:            &gimli::DebuggingInformationEntry<'_, '_, R>,
                        data_offset:    u64,
                        ) -> Result<ReturnResult<R>>
    {
        // Make sure that the die is of type DW_TAG_variant
        match die.tag() {
            gimli::DW_TAG_variant => (),
            _ => panic!("Wrong implementation"),
        };

        self.check_alignment(die, data_offset)?;

        // Find the child die of type DW_TAG_member
        let children = get_children(unit, die)?;
        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_member => {

                    // Evaluate the value of the member.
                    let value = match self.eval_member(memory_and_registers, dwarf, unit, &c_die, data_offset)? {
                        ReturnResult::Value(val) => val,
                        ReturnResult::Required(req) => return Ok(ReturnResult::Required(req)),
                    };

                    // Get the name of the die.
                    let name = match attributes::name_attribute(dwarf, &c_die) {
                        Some(val) => val,
                        None => bail!("Expected member die to have attribute DW_AT_name"),
                    };

                    return Ok(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::EnumValue {
                        name:   name,
                        value:  value,
                    }))));
                },
                _ => (),
            };
        }

        unreachable!();
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
                            bail!("Address not aligned");
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
                                           ) -> Result<Vec<gimli::UnitOffset>>
{
    let mut result = Vec::new();
    let mut tree = unit.entries_tree(Some(die.offset()))?;
    let node = tree.root()?;

    let mut children = node.children();
    while let Some(child) = children.next()? { 
        result.push(child.entry().offset());
    }
    
    Ok(result)
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
    match (encoding, byte_size) {  // Source: DWARF 4 page 168-169 and 77
        (DwAte(1), 4) => BaseValue::Address32(value as u32),    // DW_ATE_address = 1 // TODO: Different size addresses?
        (DwAte(2), 1) => BaseValue::Bool((value as u8) == 1),   // DW_ATE_boolean = 2 // TODO: Use modulus?
        
//        (DwAte(3), _) => ,   // DW_ATE_complex_float = 3 // NOTE: Seems like a C++ thing

        (DwAte(4), 4) => BaseValue::F32(f32::from_bits(value as u32)),   // DW_ATE_float = 4
        (DwAte(4), 8) => BaseValue::F64(f64::from_bits(value)), // DW_ATE_float = 4

        (DwAte(5), 1) => BaseValue::I8(value as i8),       // (DW_ATE_signed = 5, 8)
        (DwAte(5), 2) => BaseValue::I16(value as i16),     // (DW_ATE_signed = 5, 16)
        (DwAte(5), 4) => BaseValue::I32(value as i32),     // (DW_ATE_signed = 5, 32)
        (DwAte(5), 8) => BaseValue::I64(value as i64),     // (DW_ATE_signed = 5, 64)
        
//        (DwAte(6), _) => ,     // DW_ATE_signed_char = 6 // TODO: Add type

        (DwAte(7), 1) => BaseValue::U8(value as u8),       // (DW_ATE_unsigned = 7, 8)
        (DwAte(7), 2) => BaseValue::U16(value as u16),     // (DW_ATE_unsigned = 7, 16)
        (DwAte(7), 4) => BaseValue::U32(value as u32),     // (DW_ATE_unsigned = 7, 32)
        (DwAte(7), 8) => BaseValue::U64(value),            // (DW_ATE_unsigned = 7, 64)
        
//        (DwAte(8), _) => ,     // DW_ATE_unsigned_char = 8 // TODO: Add type
//        (DwAte(9), _) => ,     // DW_ATE_imaginary_float = 9 // NOTE: Seems like a C++ thing
//        (DwAte(10), _) => ,     // DW_ATE_packed_decimal = 10 // TODO: Add type
//        (DwAte(11), _) => ,     // DW_ATE_numeric_string = 11 // TODO: Add type
//        (DwAte(12), _) => ,     // DW_ATE_edited = 12 // TODO: Add type
//        (DwAte(13), _) => ,     // DW_ATE_signed_fixed = 13 // TODO: Add type
//        (DwAte(14), _) => ,     // DW_ATE_unsigned_fixed = 14 // TODO: Add type
//        (DwAte(15), _) => ,     // DW_ATE_decimal_float = 15 // TODO: Add type
//        (DwAte(16), _) => ,     // DW_ATE_UTF = 16 // TODO: Add type
//        (DwAte(128), _) => ,     // DW_ATE_lo_user = 128 // TODO: Add type
//        (DwAte(255), _) => ,     // DW_ATE_hi_user = 255 // TODO: Add type

        _ => {
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
    return ((data[1] as u64)<< 32) + (data[0] as u64);
}


/*
 * Evaluates the value of a base type.
 */
pub fn parse_base_type<R>(unit:         &gimli::Unit<R>,
                      data:         &[u32],
                      base_type:    gimli::UnitOffset<usize>
                      ) -> Result<BaseValue>
                      where R: Reader<Offset = usize>
{
    if base_type.0 == 0 {
        return Ok(BaseValue::Generic(slize_as_u64(data)));
    }
    let die = unit.entry(base_type)?;

    // I think that the die returned must be a base type tag.
    if die.tag() != gimli::DW_TAG_base_type {
        bail!("Requires at the die has tag DW_TAG_base_type");
    }

    let encoding = match die.attr_value(gimli::DW_AT_encoding)? {
        Some(gimli::AttributeValue::Encoding(dwate)) => dwate,
        _ => bail!("Expected base type die to have attribute DW_AT_encoding"),
    };
    let byte_size = match die.attr_value(gimli::DW_AT_byte_size)? {
        Some(gimli::AttributeValue::Udata(v)) => v,
        _ => bail!("Expected base type die to have attribute DW_AT_byte_size"),
    };
    
    Ok(eval_base_type(data, encoding, byte_size))
}


fn trim_piece_bytes<R: Reader<Offset = usize>>(mut bytes: Vec<u8>, piece: &Piece<R>, byte_size: usize) -> Vec<u8> {
    let piece_byte_size = match piece.size_in_bits {
        Some(size) => ((size + 8 - 1) / 8) as usize,
        None => byte_size,
    };

    let piece_byte_offset = match piece.bit_offset {
        Some(offset) => {
            if offset % 8 == 0 {
                panic!("Expected the offset to be in bytes, got {} bits", offset);
            }
            ((offset + 8 - 1) / 8) as usize
        },
        None => 0, 
    };

    for _ in 0..piece_byte_offset {
        bytes.pop();
    }

    while bytes.len() > piece_byte_size {// TODO: Cheack that this follows the ABI.
        bytes.remove(0);
    }

    return bytes;
}


/*
 * Evaluates the value of a base type.
 */
pub fn new_eval_base_type(data:         Vec<u8>,
                          encoding:     DwAte,
                          ) -> BaseValue
{
    if data.len() == 0 {
        panic!("expected byte size to be larger then 0");
    }

    match (encoding, data.len()) {  // Source: DWARF 4 page 168-169 and 77
        (DwAte(1), 4) => BaseValue::Address32(u32::from_le_bytes(data.try_into().unwrap())),    // DW_ATE_address = 1 // TODO: Different size addresses?
        (DwAte(2), 1) => BaseValue::Bool((u8::from_le_bytes(data.try_into().unwrap())) == 1),   // DW_ATE_boolean = 2 // TODO: Use modulus?
        
//        (DwAte(3), _) => ,   // DW_ATE_complex_float = 3 // NOTE: Seems like a C++ thing

        (DwAte(4), 4) => BaseValue::F32(f32::from_le_bytes(data.try_into().unwrap())),   // DW_ATE_float = 4
        (DwAte(4), 8) => BaseValue::F64(f64::from_le_bytes(data.try_into().unwrap())), // DW_ATE_float = 4

        (DwAte(5), 1) => BaseValue::I8(i8::from_le_bytes(data.try_into().unwrap())),       // (DW_ATE_signed = 5, 8)
        (DwAte(5), 2) => BaseValue::I16(i16::from_le_bytes(data.try_into().unwrap())),     // (DW_ATE_signed = 5, 16)
        (DwAte(5), 4) => BaseValue::I32(i32::from_le_bytes(data.try_into().unwrap())),     // (DW_ATE_signed = 5, 32)
        (DwAte(5), 8) => BaseValue::I64(i64::from_le_bytes(data.try_into().unwrap())),     // (DW_ATE_signed = 5, 64)
        
//        (DwAte(6), _) => ,     // DW_ATE_signed_char = 6 // TODO: Add type

        (DwAte(7), 1) => BaseValue::U8(u8::from_le_bytes(data.try_into().unwrap())),       // (DW_ATE_unsigned = 7, 8)
        (DwAte(7), 2) => BaseValue::U16(u16::from_le_bytes(data.try_into().unwrap())),     // (DW_ATE_unsigned = 7, 16)
        (DwAte(7), 4) => BaseValue::U32(u32::from_le_bytes(data.try_into().unwrap())),     // (DW_ATE_unsigned = 7, 32)
        (DwAte(7), 8) => BaseValue::U64(u64::from_le_bytes(data.try_into().unwrap())),            // (DW_ATE_unsigned = 7, 64)
        
//        (DwAte(8), _) => ,     // DW_ATE_unsigned_char = 8 // TODO: Add type
//        (DwAte(9), _) => ,     // DW_ATE_imaginary_float = 9 // NOTE: Seems like a C++ thing
//        (DwAte(10), _) => ,     // DW_ATE_packed_decimal = 10 // TODO: Add type
//        (DwAte(11), _) => ,     // DW_ATE_numeric_string = 11 // TODO: Add type
//        (DwAte(12), _) => ,     // DW_ATE_edited = 12 // TODO: Add type
//        (DwAte(13), _) => ,     // DW_ATE_signed_fixed = 13 // TODO: Add type
//        (DwAte(14), _) => ,     // DW_ATE_unsigned_fixed = 14 // TODO: Add type
//        (DwAte(15), _) => ,     // DW_ATE_decimal_float = 15 // TODO: Add type
//        (DwAte(16), _) => ,     // DW_ATE_UTF = 16 // TODO: Add type
//        (DwAte(128), _) => ,     // DW_ATE_lo_user = 128 // TODO: Add type
//        (DwAte(255), _) => ,     // DW_ATE_hi_user = 255 // TODO: Add type

        _ => {
            unimplemented!()
        },
    }
}


