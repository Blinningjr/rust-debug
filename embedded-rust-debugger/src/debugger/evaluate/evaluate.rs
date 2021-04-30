use super::{
    attributes,
    Debugger,
    eval_base_type,
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


use probe_rs::MemoryInterface;


// TODO
#[derive(Debug, PartialEq)]
pub enum NewArrayDimension {
    EnumerationType(gimli::UnitOffset),
    SubrangeType(gimli::UnitOffset),
}


// TODO: piece evaluator state.
struct EvaluatorState<R: Reader<Offset = usize>> {
    pub unit_offset:    gimli::UnitSectionOffset,
    pub die_offset:     gimli::UnitOffset,
    pub partial_value:  super::value::PartialValue<R>,
    pub data_offset:    u64,
}


impl<R: Reader<Offset = usize>> EvaluatorState<R> {
    pub fn new(unit:    &gimli::Unit<R>,
               die:    &gimli::DebuggingInformationEntry<'_, '_, R>
               ) -> EvaluatorState<R>
    {
        EvaluatorState {
            unit_offset:    unit.header.offset(),
            die_offset:     die.offset(),
            partial_value:  super::value::PartialValue::NotEvaluated,
            data_offset:    0,
        }
    }
}


pub enum EvaluatorResult {
    Complete,
    RequireReg(u16),
    RequireData {address: u32, num_words: usize},
}


pub enum ReturnResult<R: Reader<Offset = usize>> {
    Value(super::value::EvaluatorValue<R>),
    Required(EvaluatorResult),
}


pub struct Evaluator<R: Reader<Offset = usize>> {
    pieces:         Vec<Piece<R>>,
    piece_index:    usize,
    stack:          Vec<EvaluatorState<R>>,
    result:         Option<super::value::EvaluatorValue<R>>,
    registers:      std::collections::HashMap<u16, u32>,
    addresses:      std::collections::HashMap<u32, u32>,
}


impl<R: Reader<Offset = usize>> Evaluator<R> {
    pub fn new(pieces:  Vec<Piece<R>>,
               unit:    &gimli::Unit<R>,
               die:     &gimli::DebuggingInformationEntry<'_, '_, R>
               ) -> Evaluator<R>
    {
        Evaluator {
            pieces:         pieces,
            piece_index:    0,
            stack:          vec!(EvaluatorState::new(unit, die)),
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
        self.piece_index = 0;

        let state = &self.stack[0];
        
        let unit = match state.unit_offset {
            gimli::UnitSectionOffset::DebugInfoOffset(offset) => {
                let header = dwarf.debug_info.header_from_offset(offset).unwrap();
                dwarf.unit(header).unwrap()
            },
            gimli::UnitSectionOffset::DebugTypesOffset(_offset) => {
                let mut iter = dwarf.debug_types.units();
                let mut result = None;
                while let Some(header) = iter.next().unwrap() {
                    if header.offset() == state.unit_offset {
                        result = Some(dwarf.unit(header).unwrap());
                        break;
                    }
                }
                result.unwrap()
            },
        };

        let die = &unit.entry(state.die_offset).unwrap();

        match self.eval_type(dwarf, &unit, die, 0).unwrap().unwrap() {
            ReturnResult::Value(val) => self.result = Some(val),
            ReturnResult::Required(req) => return req,
        };

        EvaluatorResult::Complete
    }


    pub fn get_value(self) -> Option<super::value::EvaluatorValue<R>> {
        self.result
    }


    pub fn get_type_info(&mut self,
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


    pub fn eval_type(&mut self,
                     dwarf:         &gimli::Dwarf<R>,
                     unit:          &gimli::Unit<R>,
                     die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                     data_offset:   u64
                     ) -> Result<Option<ReturnResult<R>>>
    { 
        match die.tag() {
            gimli::DW_TAG_base_type                 => self.eval_basetype(unit, die, data_offset),
            gimli::DW_TAG_pointer_type              => self.eval_pointer_type(unit, die, data_offset),
            gimli::DW_TAG_array_type                => self.eval_array_type(dwarf, unit, die, data_offset, false), // TODO: fix the bool
            gimli::DW_TAG_structure_type            => self.eval_structured_type(dwarf, unit, die, data_offset, false),
            gimli::DW_TAG_union_type                => self.eval_union_type(dwarf, unit, die, data_offset, false),
            gimli::DW_TAG_member                    => self.eval_member(dwarf, unit, die, data_offset),
            gimli::DW_TAG_enumeration_type          => self.eval_enumeration_type(dwarf, unit, die, data_offset),
            gimli::DW_TAG_string_type               => unimplemented!(),
            gimli::DW_TAG_generic_subrange          => unimplemented!(),
            gimli::DW_TAG_template_type_parameter   => unimplemented!(),
            gimli::DW_TAG_variant_part              => self.eval_variant_part(dwarf, unit, die, data_offset),
            gimli::DW_TAG_subroutine_type           => unimplemented!(),
            gimli::DW_TAG_subprogram                => unimplemented!(),
            _ => unimplemented!(),
        }
    }


    pub fn eval_piece(&mut self,
                      piece:        Piece<R>,
                      byte_size:    Option<u64>,
                      data_offset:  u64,
                      encoding:     Option<DwAte>
                      ) -> Option<ReturnResult<R>>
    {
        return match piece.location {
            Location::Empty                                         => Some(ReturnResult::Value(super::value::EvaluatorValue::OptimizedOut)),
            Location::Register        { register }                  => self.eval_register(register),
            Location::Address         { address }                   => self.eval_address(address, byte_size, data_offset, encoding.unwrap()),
            Location::Value           { value }                     => Some(ReturnResult::Value(super::value::EvaluatorValue::Value(super::value::convert_from_gimli_value_new(value)))),
            Location::Bytes           { value }                     => Some(ReturnResult::Value(super::value::EvaluatorValue::Bytes(value))),
            Location::ImplicitPointer { value: _, byte_offset: _ }  => unimplemented!(),
        };
    }


    pub fn eval_register(&mut self,
                         register:  gimli::Register
                         ) -> Option<ReturnResult<R>>
    {
        match self.registers.get(&register.0) {
            Some(val) => Some(ReturnResult::Value(super::value::EvaluatorValue::Value(super::value::BaseValue::U32(*val)))), // TODO: Mask the important bits?
            None    => Some(ReturnResult::Required(EvaluatorResult::RequireReg(register.0))),
        }
    }


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

        println!("Address: {:#10x}", address);
        println!("data_offset: {}", data_offset);
        address += (data_offset/4) * 4;
        println!("Address: {:#10x}", address);

        //address -= address%4; // TODO: Is this correct?


        let mut data: Vec<u32> = Vec::new();
        for i in 0..num_words as usize {
            match self.addresses.get(&((address + (i as u64) * 2) as u32)) {
                Some(val) => data.push(*val), // TODO: Mask the important bits?
                None    => return Some(ReturnResult::Required(EvaluatorResult::RequireData{ address: (address + (i as u64) * 2) as u32, num_words: 1 })),
            }
        }

        Some(ReturnResult::Value(
                super::value::EvaluatorValue::Value(
                    eval_base_type(&data, encoding, byte_size.unwrap()))))

//        Some(ReturnResult::Required(EvaluatorResult::RequireData {address: address as u32, num_words: num_words as usize}))
    }


    pub fn handle_eval_piece(&mut self,
                             byte_size:         Option<u64>,
                             mut data_offset:   u64,
                             encoding:          Option<DwAte>
                             ) -> Result<Option<ReturnResult<R>>>
    {
        if self.pieces.len() <= self.piece_index {
            return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::OptimizedOut)));
        }
        
        if self.pieces.len() > 1 {
            data_offset = 0;
        }
        
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


    pub fn eval_basetype(&mut self,
                         unit:          &gimli::Unit<R>,
                         die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                         data_offset:   u64
                         ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_base_type => (),
            _ => panic!("Wrong implementation"),
        };

        let byte_size = attributes::byte_size_attribute(die);
        let encoding =  attributes::encoding_attribute(die);
        match byte_size {
            Some(0) => return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::ZeroSize))),
            _       => (),
        };

        self.handle_eval_piece(byte_size,
                               data_offset, // TODO
                               encoding)
    }


    pub fn eval_pointer_type(&mut self,
                             unit:          &gimli::Unit<R>,
                             die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                             data_offset:   u64
                             ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_pointer_type => (),
            _ => panic!("Wrong implementation"),
        };

        let address_class = attributes::address_class_attribute(die);

        match address_class.unwrap().0 {
            0 => {
                let res = self.handle_eval_piece(Some(4),
                                                 data_offset,
                                                 Some(DwAte(1)));
                return res;        
            },
            _ => panic!("Unimplemented DwAddr code"), // NOTE: The codes are architecture specific.
        };
    }


    pub fn eval_array_type(&mut self,
                           dwarf:       &gimli::Dwarf<R>,
                           unit:        &gimli::Unit<R>,
                           die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                           data_offset: u64,
                           new_state:   bool
                           ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_array_type => (),
            _ => panic!("Wrong implementation"),
        };

        //TODO: Add the state and pop it.
        //let mut current_state = self.stack.len() - 1;

        //if new_state {
        //    self.stack.push(EvaluatorState::new(unit, die));
        //    current_state += 1;

        //    self.stack[current_state].data_offset = data_offset;
        //    self.stack[current_state].partial_value = super::value::PartialValue::Array(Box::new(super::value::PartialArrayValue { values: vec!() }));
        //} 

        let children = get_children(unit, die);
        let dimension_die = unit.entry(children[0])?;
        let array_len_result = match dimension_die.tag() {
            gimli::DW_TAG_subrange_type     => self.eval_subrange_type(dwarf, unit, &dimension_die, data_offset)?.unwrap(),
            gimli::DW_TAG_enumeration_type  => self.eval_enumeration_type(dwarf, unit, &dimension_die, data_offset)?.unwrap(),
            _ => unimplemented!(),
        };
        
        let count = match array_len_result {
            ReturnResult::Value(val) => super::value::get_udata_new(val.to_value().unwrap()),
            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
        } as usize;

        let mut partial_array = super::value::PartialArrayValue { values: vec!() };

        //let mut partial_array = match &self.stack[current_state].partial_value {
        //    super::value::PartialValue::Array   (array) => array.clone(),
        //    _ => return Err(anyhow!("Critical Error: expected parital array")),
        //};

        let start = partial_array.values.len();

        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;

        for _i in start..count {
            match self.eval_type(dwarf, &type_unit, type_die, data_offset)?.unwrap() { // TODO: Fix so that it can read multiple of the same type.
                ReturnResult::Value(val) => partial_array.values.push(val),
                ReturnResult::Required(req) => {
                    //self.stack[current_state].partial_value = super::value::PartialValue::Array(Box::new(partial_value));
                    return Ok(Some(ReturnResult::Required(req)));
                },
            };
        }
        

        //self.stack.pop();
        Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Array(Box::new(super::value::NewArrayValue {values: partial_array.values})))))
    }


    pub fn eval_structured_type(&mut self,
                                dwarf:       &gimli::Dwarf<R>,
                                unit:        &gimli::Unit<R>,
                                die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                                data_offset: u64,
                                new_state:   bool
                                ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_structure_type => (),
            _ => panic!("Wrong implementation"),
        };

        let name = attributes::name_attribute(dwarf, die).unwrap();

        let children = get_children(unit, die);

        let mut member_dies = Vec::new();
        for c in &children {
            let c_die = unit.entry(*c)?;
            match c_die.tag() {
                gimli::DW_TAG_variant_part => {

                    let members = vec!(match self.eval_variant_part(dwarf, unit, &c_die, data_offset)?.unwrap() {
                        ReturnResult::Value(val) => val,
                        ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                    });

                    return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Struct(Box::new(super::value::NewStructValue {
                        name:       name,
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

        member_dies.sort_by_key(|m| m.0);

        let mut members = vec!();
        for (_, m) in &member_dies {
            let member = match self.eval_member(dwarf, unit, m, data_offset)?.unwrap() {
                ReturnResult::Value(val) => val,
                ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
            };
            members.push(member);
        }

        return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Struct(Box::new(super::value::NewStructValue {
            name:       name,
            members:    members,
        })))));
    }


    pub fn eval_union_type(&mut self,
                           dwarf:       &gimli::Dwarf<R>,
                           unit:        &gimli::Unit<R>,
                           die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                           data_offset: u64,
                           new_state:   bool
                           ) -> Result<Option<ReturnResult<R>>>
    { 
        match die.tag() {
            gimli::DW_TAG_union_type => (),
            _ => panic!("Wrong implementation"),
        };

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
 
        member_dies.sort_by_key(|m| m.0);

        let mut members = vec!();
        for (_, m) in &member_dies {
            let member = match self.eval_member(dwarf, unit, m, data_offset)?.unwrap() {
                ReturnResult::Value(val) => val,
                ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
            };
            members.push(member);
        }

        let name = attributes::name_attribute(dwarf, die).unwrap();

        return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Union(Box::new(super::value::NewUnionValue {
            name:       name,
            members:    members,
        })))));
    }


    pub fn eval_member(&mut self,
                       dwarf:       &gimli::Dwarf<R>,
                       unit:        &gimli::Unit<R>,
                       die:         &gimli::DebuggingInformationEntry<'_, '_, R>,
                       data_offset: u64,
                       ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_member => (),
            _ => panic!("Wrong implementation"),
        };

        let new_data_offset = match attributes::data_member_location_attribute(die) {
            Some(val)   => data_offset + val,
            None        => data_offset,
        };

        let name = attributes::name_attribute(dwarf, die);

        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;
        
        let value = match self.eval_type(dwarf, &type_unit, type_die, new_data_offset)?.unwrap() {
            ReturnResult::Value(val) => val,
            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
        };

        Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Member(Box::new(super::value::NewMemberValue{
            name:   name,
            value:  value
        })))))
    }


    pub fn eval_enumeration_type(&mut self,
                                 dwarf:         &gimli::Dwarf<R>,
                                 unit:          &gimli::Unit<R>,
                                 die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                                 data_offset:   u64
                                 ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_enumeration_type => (),
            _ => panic!("Wrong implementation"),
        };

        // TODO: Create new evaluator state.

        let (type_unit, die_offset) = self.get_type_info(dwarf, unit, die)?;
        let type_die = &type_unit.entry(die_offset)?;

        let type_result = match self.eval_type(dwarf, &type_unit, type_die, data_offset)?.unwrap() {
            ReturnResult::Value(val) => val,
            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
        };
        let value = super::value::get_udata_new(type_result.to_value().unwrap());

        let children = get_children(unit, die);

        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_enumerator  => {
                    let const_value = attributes::const_value_attribute(&c_die).unwrap();
        
                    if const_value == value {
                        let name = attributes::name_attribute(dwarf, die).unwrap();
        
                        let e_name = attributes::name_attribute(dwarf, &c_die).unwrap(); 
        
                        return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::NewEnumValue {
                            name:   name,
                            value:  super::value::EvaluatorValue::Name(e_name),
                        })))));
                    }
                },
                gimli::DW_TAG_subprogram => (),
                _ => unimplemented!(),
            };
        }

        Ok(None)
    }


    pub fn eval_subrange_type(&mut self,
                              dwarf:        &gimli::Dwarf<R>,
                              unit:          &gimli::Unit<R>,
                              die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                              data_offset:   u64
                              ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_subrange_type => (),
            _ => panic!("Wrong implementation"),
        };

        match attributes::count_attribute(die) {
            Some(val)   => return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Value(super::value::BaseValue::U64(val))))),
            None        => (),
        };

        let (type_unit, die_offset) = match self.get_type_info(dwarf, unit, die) {
            Ok(val) => val,
            Err(_) => return Ok(None),
        };
        let type_die = &type_unit.entry(die_offset)?;

        self.eval_type(dwarf, &type_unit, type_die, data_offset)
    }


    pub fn eval_variant_part(&mut self,
                             dwarf:         &gimli::Dwarf<R>,
                             unit:          &gimli::Unit<R>,
                             die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                             data_offset:   u64
                             ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_variant_part => (),
            _ => panic!("Wrong implementation"),
        };

        let mut children = get_children(unit, die);
        let mut member = None;
        let mut variants = vec!();
        for c in &children {
            let c_die = unit.entry(*c)?;
            match c_die.tag() {
                gimli::DW_TAG_member => {
                    if member.is_some() {
                        panic!("Expacted only one member");
                    }
                    member = Some(c_die);
                },
                gimli::DW_TAG_variant => variants.push(c_die),
                _ => (),
            };
        }

        match &member {
            Some    (member)   => {
                let variant = match self.eval_member(dwarf, unit, member, data_offset)?.unwrap() {
                    ReturnResult::Value(val) => super::value::get_udata_new(val.to_value().unwrap()),
                    ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                };


                for v in &variants {
                    let discr_value = attributes::discr_value_attribute(v).unwrap();

                    if discr_value == variant {

                        return self.eval_variant(dwarf, unit, v, data_offset);

                        //return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::NewEnumValue{
                        //    name:   v.member.name.clone().unwrap(),
                        //    value:  self.eval_member(core, pieces, index, data_offset, &v.member)?.unwrap(),
                        //}))));
                    }
                }
                unimplemented!();
            },
            None            => {
                unimplemented!();
            },
        };
    }


    pub fn eval_variant(&mut self,
                        dwarf:         &gimli::Dwarf<R>,
                        unit:          &gimli::Unit<R>,
                        die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                        data_offset:   u64
                        ) -> Result<Option<ReturnResult<R>>>
    {
        match die.tag() {
            gimli::DW_TAG_variant => (),
            _ => panic!("Wrong implementation"),
        };

        let mut children = get_children(unit, die);
        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_member => {
                    let value = match self.eval_member(dwarf, unit, &c_die, data_offset)?.unwrap() {
                        ReturnResult::Value(val) => val,
                        ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
                    };
                    let name = attributes::name_attribute(dwarf, &c_die).unwrap();

                    return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::NewEnumValue {
                        name:   name,
                        value:  value,
                    })))));
                },
                _ => (),
            };
        }
        unimplemented!();
    }
}


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






/////////////////////////////////////////////////////////////////////////////


// TODO: Add none type evaluator
impl<R: Reader<Offset = usize>> Debugger<R> {
    pub fn eval_piece(&mut self,
                      core:         &mut probe_rs::Core,
                      piece:        Piece<R>,
                      byte_size:    Option<u64>,
                      data_offset:  u64,
                      encoding:     Option<DwAte>
                      ) -> Result<Option<super::value::EvaluatorValue<R>>>
    {
        //println!("{:#?}", piece);

        return match piece.location {
            Location::Empty                                         => Ok(Some(super::value::EvaluatorValue::OptimizedOut)),
            Location::Register        { register }                  => self.eval_register(core, register),
            Location::Address         { address }                   => self.eval_address(core, address, byte_size, data_offset, encoding.unwrap()),
            Location::Value           { value }                     => Ok(Some(super::value::EvaluatorValue::Value(super::value::convert_from_gimli_value_new(value)))),
            Location::Bytes           { value }                     => Ok(Some(super::value::EvaluatorValue::Bytes(value))),
            Location::ImplicitPointer { value: _, byte_offset: _ }  => unimplemented!(),
        };
    }


    pub fn eval_register(&mut self,
                         core:      &mut probe_rs::Core,
                         register:  gimli::Register
                         ) -> Result<Option<super::value::EvaluatorValue<R>>>
    {
        let data = core.read_core_reg(register.0)?;
        return Ok(Some(super::value::EvaluatorValue::Value(super::value::BaseValue::U32(data)))); // TODO: Mask the important bits?
    }


    pub fn eval_address(&mut self,
                        core:           &mut probe_rs::Core,
                        mut address:    u64,
                        byte_size:      Option<u64>,
                        data_offset:    u64,
                        encoding:       DwAte
                        ) -> Result<Option<super::value::EvaluatorValue<R>>>
    {
        let num_words = match byte_size {
            Some(val)   => (val + 4 - 1 )/4,
            None        => 1,
        };

        println!("Address: {:#10x}", address);
        println!("data_offset: {}", data_offset);
        address += (data_offset/4) * 4;
        println!("Address: {:#10x}", address);

        //address -= address%4; // TODO: Is this correct?

        let mut data: Vec<u32> = vec![0; num_words as usize];
        core.read_32(address as u32, &mut data)?;

        let mut res: Vec<u32> = Vec::new();
        for d in data.iter() {
            res.push(*d);
        }

        Ok(Some(super::value::EvaluatorValue::Value(
                    eval_base_type(&data, encoding, byte_size.unwrap()))))
    }
}

