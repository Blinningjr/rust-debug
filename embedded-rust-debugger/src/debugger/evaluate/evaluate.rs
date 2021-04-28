use super::{
    attributes,
    Debugger,
    DebuggerValue,
    eval_base_type,
    EnumValue,
    value::{
        get_udata,
        StructValue,
        MemberValue,
        UnionValue,
        ArrayValue,
        Value,
        convert_from_gimli_value,
    },
};


use crate::debugger::types::types::{
    DebuggerType,
    BaseType,
    MemberType,
    StructuredType,
    VariantPart,
    EnumerationType,
    Enumerator,
    UnionType,
    ArrayType,
    ArrayDimension,
    SubrangeType,
    PointerType,
    TemplateTypeParameter,
    StringType,
    SubroutineType,
    Subprogram,
    GenericSubrangeType,
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


enum EvaluatorResult {
    Complete,
    RequireReg(u16),
    RequireData {address: u32, num_words: usize},
}

enum ReturnResult<R: Reader<Offset = usize>> {
    Value(super::value::EvaluatorValue<R>),
    Required(EvaluatorResult),
}



struct Evaluator<R: Reader<Offset = usize>> {
    pieces:         Vec<Piece<R>>,
    piece_index:    usize,
    stack:          Vec<EvaluatorState<R>>,
    result:         Option<super::value::EvaluatorValue<R>>,
    // TODO: Add hashmap for registers maybe?
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
        }
    }


    pub fn evaluate(&mut self) -> EvaluatorResult {

        EvaluatorResult::Complete
    }


    pub fn get_value(self) -> Option<super::value::EvaluatorValue<R>> {
        self.result
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
        Some(ReturnResult::Required(EvaluatorResult::RequireReg(register.0)))
        // TODO
        //let data = core.read_core_reg(register.0)?;
        //return Ok(Some(DebuggerValue::Value(Value::U32(data)))); // TODO: Mask the important bits?
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



        Some(ReturnResult::Required(EvaluatorResult::RequireData {address: address as u32, num_words: num_words as usize}))
        // TODO
        //let mut data: Vec<u32> = vec![0; num_words as usize];
        //core.read_32(address as u32, &mut data)?;

        //let mut res: Vec<u32> = Vec::new();
        //for d in data.iter() {
        //    res.push(*d);
        //}

        //return Ok(Some(DebuggerValue::Value(eval_base_type(&data, encoding, byte_size.unwrap()))));
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
// TODO: Should only pop piece if value is correctly evaluated
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

        return Ok(res);
    }


    pub fn eval_basetype(&mut self,
                         unit:          &gimli::Unit<R>,
                         die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                         data_offset:   u64
                         ) -> Result<Option<ReturnResult<R>>>
    {
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
        let address_class = attributes::address_class_attribute(die);

        match address_class.unwrap().0 { // TODO: remove unwrap and the option around address_type.
            0 => {
                let res = self.handle_eval_piece(Some(4),
                                                 data_offset, // TODO
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
        fn get_dimensions<R: Reader<Offset = usize>>(unit: &gimli::Unit<R>, die: &gimli::DebuggingInformationEntry<'_, '_, R>) -> Vec<NewArrayDimension> {
            let mut dimensions  = Vec::new();
            let mut tree = unit.entries_tree(Some(die.offset())).unwrap();
            let node = tree.root().unwrap();

            let mut children = node.children();
            if let Some(child) = children.next().unwrap() { 
                let c_die = child.entry();
                match c_die.tag() {
                    gimli::DW_TAG_subrange_type     => dimensions.push(NewArrayDimension::SubrangeType(c_die.offset())),
                    gimli::DW_TAG_enumeration_type  => dimensions.push(NewArrayDimension::EnumerationType(c_die.offset())),
                    _ => {
                        unimplemented!(); //TODO: Add parser for generic_subrange.
                    },
                };
            }
            
            dimensions
        }

        //let mut current_state = self.stack.len() - 1;

        //if new_state {
        //    self.stack.push(EvaluatorState::new(unit, die));
        //    current_state += 1;

        //    self.stack[current_state].data_offset = data_offset;
        //    self.stack[current_state].partial_value = super::value::PartialValue::Array(Box::new(super::value::PartialArrayValue { values: vec!() }));
        //}

        let dimensions = get_dimensions(unit, die);

        let array_len_result = match &dimensions[0] {
            NewArrayDimension::EnumerationType(die_offset) => {
                let new_die = unit.entry(*die_offset)?;
                self.eval_enumeration_type(dwarf, unit, &new_die, data_offset)?.unwrap()
            },
            NewArrayDimension::SubrangeType(die_offset) => {
                let new_die = unit.entry(*die_offset)?;
                self.eval_subrange_type(dwarf, unit, &new_die, data_offset)?.unwrap()
            },
        };

        let count = match array_len_result {
            ReturnResult::Value(val) => super::value::get_udata_new(val.to_value().unwrap()),
            ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
        };

        let mut partial_array = super::value::PartialArrayValue { values: vec!() };

        //let mut partial_array = match &self.stack[current_state].partial_value {
        //    super::value::PartialValue::Array   (array) => array.clone(),
        //    _ => return Err(anyhow!("Critical Error: expected parital array")),
        //};

        let start = partial_array.values.len();

        // TODO: Get type die and unit.
//        for _i in start..count {
//            match self.eval_type(data_offset, &array_type.r#type)?.unwrap() { // TODO: Fix so that it can read multiple of the same type.
//                ReturnResult::Value(val) => partial_array.values.push(val),
//                ReturnResult::Required(er) => {
//                    //self.stack[current_state].partial_value = super::value::PartialValue::Array(Box::new(partial_value));
//                    return ReturnResult::Required(er);
//                },
//            };
//        }
        

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
        fn get_children<R: Reader<Offset = usize>>(unit: &gimli::Unit<R>, die: &gimli::DebuggingInformationEntry<'_, '_, R>) -> Vec<gimli::UnitOffset> {
            let mut result = Vec::new();
            let mut tree = unit.entries_tree(Some(die.offset())).unwrap();
            let node = tree.root().unwrap();

            let mut children = node.children();
            if let Some(child) = children.next().unwrap() { 
                result.push(child.entry().offset());
            }
            
            result
        }

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
        fn get_children<R: Reader<Offset = usize>>(unit: &gimli::Unit<R>, die: &gimli::DebuggingInformationEntry<'_, '_, R>) -> Vec<gimli::UnitOffset> {
            let mut result = Vec::new();
            let mut tree = unit.entries_tree(Some(die.offset())).unwrap();
            let node = tree.root().unwrap();

            let mut children = node.children();
            if let Some(child) = children.next().unwrap() { 
                result.push(child.entry().offset());
            }
            
            result
        }
        
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
        let new_data_offset = match attributes::data_member_location_attribute(die) {
            Some(val)   => data_offset + val,
            None        => data_offset,
        };

        let name = attributes::name_attribute(dwarf, die).unwrap();

        // TODO: Get type die.
        //let value = match self.eval_type(type_core, type_die, new_data_offset)?.unwrap() {
        //    ReturnResult::Value(val) => val,
        //    ReturnResult::Required(req) => return Ok(Some(ReturnResult::Required(req))),
        //};

        //Ok(Some(ReturnResult::Value(EvaluatorValue::Member(Box::new(super::value::NewMemberValue{
        //    name:   name,
        //    value:  value
        //})))))

        Ok(None)
    }


    pub fn eval_enumeration_type(&mut self,
                                 dwarf:         &gimli::Dwarf<R>,
                                 unit:          &gimli::Unit<R>,
                                 die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                                 data_offset:   u64
                                 ) -> Result<Option<ReturnResult<R>>>
    {
        fn get_enumerations<R: Reader<Offset = usize>>(unit: &gimli::Unit<R>, die: &gimli::DebuggingInformationEntry<'_, '_, R>) -> Vec<gimli::UnitOffset> {
            let mut enumerators = Vec::new();
            let mut tree = unit.entries_tree(Some(die.offset())).unwrap();
            let node = tree.root().unwrap();

            let mut children = node.children();
            if let Some(child) = children.next().unwrap() { 
                let c_die = child.entry();
                match c_die.tag() {
                    gimli::DW_TAG_enumerator  => enumerators.push(c_die.offset()),
                    gimli::DW_TAG_subprogram => (),
                    _ => unimplemented!(),
                };
            }
            
            enumerators
        }
        // TODO: Create new evaluator state.
        // TODO: get type unit and die.
        //let value = get_udata(self.eval_type(type_unit, type_die, data_offset)?.unwrap().to_value().unwrap());
        let value = 0;
        
        let enumerations = get_enumerations(unit, die);

        for e in &enumerations {
            let e_die = unit.entry(*e)?;

            let const_value = attributes::const_value_attribute(&e_die).unwrap();

            if const_value == value {
                let name = attributes::name_attribute(dwarf, die).unwrap();

                let e_name = attributes::name_attribute(dwarf, &e_die).unwrap(); 

                return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Enum(Box::new(super::value::NewEnumValue {
                    name:   name,
                    value:  super::value::EvaluatorValue::Name(e_name),
                })))));
            }
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
        match attributes::count_attribute(die) {
            Some(val)   => return Ok(Some(ReturnResult::Value(super::value::EvaluatorValue::Value(super::value::BaseValue::U64(val))))),
            None        => (),
        };

        // TODO: Get type die.
//        match &*subrange_type.r#type {
//            Some(val)   => return self.eval_type(unit, type_die, data_offset),
//            None        => (),
//        };

        Ok(None)
    }


    pub fn eval_variant_part(&mut self,
                             dwarf:         &gimli::Dwarf<R>,
                             unit:          &gimli::Unit<R>,
                             die:           &gimli::DebuggingInformationEntry<'_, '_, R>,
                             data_offset:   u64
                             ) -> Result<Option<ReturnResult<R>>>
    {
        fn get_children<R: Reader<Offset = usize>>(unit: &gimli::Unit<R>, die: &gimli::DebuggingInformationEntry<'_, '_, R>) -> Vec<gimli::UnitOffset> {
            let mut result = Vec::new();
            let mut tree = unit.entries_tree(Some(die.offset())).unwrap();
            let node = tree.root().unwrap();

            let mut children = node.children();
            if let Some(child) = children.next().unwrap() { 
                result.push(child.entry().offset());
            }
            
            result
        }

        let mut children = get_children(unit, die);
        let mut member = None;
        let mut variants = vec!();
        for c in children {
            let c_die = unit.entry(c)?;
            match c_die.tag() {
                gimli::DW_TAG_member => {
                    if member.is_none() {
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

                        return self.eval_variant(dwarf, unit, die, data_offset);

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
        fn get_children<R: Reader<Offset = usize>>(unit: &gimli::Unit<R>, die: &gimli::DebuggingInformationEntry<'_, '_, R>) -> Vec<gimli::UnitOffset> {
            let mut result = Vec::new();
            let mut tree = unit.entries_tree(Some(die.offset())).unwrap();
            let node = tree.root().unwrap();

            let mut children = node.children();
            if let Some(child) = children.next().unwrap() { 
                result.push(child.entry().offset());
            }
            
            result
        }

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






impl<R: Reader<Offset = usize>> Debugger<R> {
    pub fn eval_type(&mut self,
                     core:          &mut probe_rs::Core,
                     pieces:        &mut Vec<Piece<R>>,
                     index:         &mut usize,
                     data_offset:   u64,
                     dtype:         &DebuggerType
                     ) -> Result<Option<DebuggerValue<R>>>
    {
        return match dtype {
            DebuggerType::BaseType              (bt)    => self.eval_basetype(core, pieces, index, data_offset, bt),
            DebuggerType::PointerType           (pt)    => self.eval_pointer_type(core, pieces, index, data_offset, pt),
            DebuggerType::ArrayType             (at)    => self.eval_array_type(core, pieces, index, data_offset, at),
            DebuggerType::StructuredType        (st)    => self.eval_structured_type(core, pieces, index, data_offset, st),
            DebuggerType::UnionType             (ut)    => self.eval_union_type(core, pieces, index, data_offset, ut),
            DebuggerType::MemberType            (mt)    => self.eval_member(core, pieces, index, data_offset, mt),
            DebuggerType::EnumerationType       (et)    => self.eval_enumeration_type(core, pieces, index, data_offset, et),
            DebuggerType::StringType            (st)    => self.eval_string_type(pieces, index, data_offset, st),
            DebuggerType::GenericSubrangeType   (gt)    => self.eval_generic_subrange_type(pieces, index, data_offset, gt),
            DebuggerType::TemplateTypeParameter (tp)    => self.eval_template_type_parameter(pieces, index, data_offset, tp),
            DebuggerType::VariantPart           (vp)    => self.eval_variant_part(core, pieces, index, data_offset, vp),
            DebuggerType::SubroutineType        (st)    => self.eval_subroutine_type(pieces, index, data_offset, st),
            DebuggerType::Subprogram            (sp)    => self.eval_subprogram(pieces, index, data_offset, sp),
        };
    }


    pub fn eval_piece(&mut self,
                      core:         &mut probe_rs::Core,
                      piece:        Piece<R>,
                      byte_size:    Option<u64>,
                      data_offset:  u64,
                      encoding:     Option<DwAte>
                      ) -> Result<Option<DebuggerValue<R>>>
    {
        //println!("{:#?}", piece);

        return match piece.location {
            Location::Empty                                         => Ok(Some(DebuggerValue::OptimizedOut)),
            Location::Register        { register }                  => self.eval_register(core, register),
            Location::Address         { address }                   => self.eval_address(core, address, byte_size, data_offset, encoding.unwrap()),
            Location::Value           { value }                     => Ok(Some(DebuggerValue::Value(convert_from_gimli_value(value)))),
            Location::Bytes           { value }                     => Ok(Some(DebuggerValue::Bytes(value))),
            Location::ImplicitPointer { value: _, byte_offset: _ }  => unimplemented!(),
        };
    }


    pub fn eval_register(&mut self,
                         core:      &mut probe_rs::Core,
                         register:  gimli::Register
                         ) -> Result<Option<DebuggerValue<R>>>
    {
        let data = core.read_core_reg(register.0)?;
        return Ok(Some(DebuggerValue::Value(Value::U32(data)))); // TODO: Mask the important bits?
    }


    pub fn eval_address(&mut self,
                        core:           &mut probe_rs::Core,
                        mut address:    u64,
                        byte_size:      Option<u64>,
                        data_offset:    u64,
                        encoding:       DwAte
                        ) -> Result<Option<DebuggerValue<R>>>
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

        return Ok(Some(DebuggerValue::Value(eval_base_type(&data, encoding, byte_size.unwrap()))));
    }


    pub fn eval_basetype(&mut self,
                         core:          &mut probe_rs::Core,
                         pieces:        &mut Vec<Piece<R>>,
                         index:         &mut usize,
                         data_offset:   u64,
                         base_type:     &BaseType
                         ) -> Result<Option<DebuggerValue<R>>>
    {
        match base_type.byte_size {
            Some(0) => return Ok(Some(DebuggerValue::ZeroSize)),
            _       => (),
        };

        let res = self.handle_eval_piece(core,
                                         pieces,
                                         index,
                                         base_type.byte_size,
                                         data_offset, // TODO
                                         Some(base_type.encoding));

        return res;
    }


    pub fn eval_pointer_type(&mut self,
                             core:          &mut probe_rs::Core,
                             pieces:        &mut Vec<Piece<R>>,
                             index:         &mut usize,
                             data_offset:   u64,
                             pointer_type:  &PointerType,
                             ) -> Result<Option<DebuggerValue<R>>>
    {
        match pointer_type.address_class.unwrap().0 { // TODO: remove unwrap and the option around address_type.
            0 => {
                let res = self.handle_eval_piece(core,
                                                 pieces,
                                                 index,
                                                 Some(4),
                                                 data_offset, // TODO
                                                 Some(DwAte(1)));
                return res;        
            },
            _ => panic!("Unimplemented DwAddr code"), // NOTE: The codes are architecture specific.
        };
    }


    pub fn eval_array_type(&mut self,
                           core:        &mut probe_rs::Core,
                           pieces:      &mut Vec<Piece<R>>,
                           index:       &mut usize,
                           data_offset: u64,
                           array_type:  &ArrayType
                           ) -> Result<Option<DebuggerValue<R>>>
    {
        let count = get_udata(match &array_type.dimensions[0] {
            ArrayDimension::EnumerationType (et)    => self.eval_enumeration_type(core, pieces, index, data_offset, et),
            ArrayDimension::SubrangeType    (st)    => self.eval_subrange_type(core, pieces, index, data_offset, st),
        }?.unwrap().to_value().unwrap());

        let mut values = Vec::new();
        for _i in 0..count {
            values.push(self.eval_type(core,
                                       pieces,
                                       index,
                                       data_offset,
                                       &array_type.r#type)?.unwrap());  // TODO: Fix so that it can read multiple of the same type.
        }
        
        return Ok(Some(DebuggerValue::Array(Box::new(ArrayValue{
            values: values,
        }))));
    }


    pub fn eval_structured_type(&mut self,
                                core:               &mut probe_rs::Core,
                                pieces:             &mut Vec<Piece<R>>,
                                index:              &mut usize,
                                data_offset:        u64,
                                structured_type:    &StructuredType
                                ) -> Result<Option<DebuggerValue<R>>>
    {
        let mut members = Vec::new();
        for c in &structured_type.children {
            match &(**c) {
                DebuggerType::VariantPart   (vp)    => {
                    let members = vec!(self.eval_variant_part(core, pieces, index, data_offset, &vp)?.unwrap());

                    return Ok(Some(DebuggerValue::Struct(Box::new(StructValue{
                        name:       structured_type.name.clone().unwrap(),
                        members:    members,
                    }))));
                },

                DebuggerType::MemberType    (mt)    => {
                    members.push(mt);
                },
                _ => continue,
            };
        }

        members.sort_by_key(|m| m.data_member_location);
        let members = members.into_iter().map(|m| self.eval_member(core, pieces, index, data_offset, m).unwrap().unwrap()).collect();

        return Ok(Some(DebuggerValue::Struct(Box::new(StructValue{
            name:       structured_type.name.clone().unwrap(),
            members:    members,
        }))));
    }


    pub fn eval_union_type(&mut self,
                           core:        &mut probe_rs::Core,
                           pieces:      &mut Vec<Piece<R>>,
                           index:       &mut usize,
                           data_offset: u64,
                           union_type:  &UnionType
                           ) -> Result<Option<DebuggerValue<R>>>
    {
        let mut members = Vec::new();
        for c in &union_type.children {
            match &(**c) {
                DebuggerType::MemberType    (mt)    => {
                    members.push(mt);
                },
                _ => continue,
            };
        }

        members.sort_by_key(|m| m.data_member_location);
        let members = members.into_iter().map(|m| self.eval_member(core, pieces, index, data_offset, m).unwrap().unwrap()).collect();

        return Ok(Some(DebuggerValue::Union(Box::new(UnionValue{
            name:       union_type.name.clone().unwrap(),
            members:    members,
        }))));
    }


    pub fn eval_member(&mut self,
                       core:            &mut probe_rs::Core,
                       pieces:          &mut Vec<Piece<R>>,
                       index:           &mut usize,
                       mut data_offset: u64,
                       member:          &MemberType
                       ) -> Result<Option<DebuggerValue<R>>>
    {
        match member.data_member_location {
            Some(val)   => data_offset += val,
            None        => (),
        };

        return Ok(Some(DebuggerValue::Member(Box::new(MemberValue{
            name:   member.name.clone(),
            value:  self.eval_type(core, pieces, index, data_offset, &member.r#type)?.unwrap(),
        }))));
    }


    pub fn eval_enumeration_type(&mut self,
                                 core:              &mut probe_rs::Core,
                                 pieces:            &mut Vec<Piece<R>>,
                                 index:             &mut usize,
                                 data_offset:       u64,
                                 enumeration_type:  &EnumerationType
                                 ) -> Result<Option<DebuggerValue<R>>>
    {
        let value = get_udata(self.eval_type(core,
                                             pieces,
                                             index,
                                             data_offset,
                                             (*enumeration_type.r#type).as_ref().unwrap())?.unwrap().to_value().unwrap());

        for e in &enumeration_type.enumerations {
            if e.const_value == value {
                return Ok(Some(DebuggerValue::Enum(Box::new(EnumValue{
                    name:   enumeration_type.name.clone().unwrap(),
                    value:  DebuggerValue::Name(e.name.clone()),
                }))));
            }
        }

        Ok(None)
    }


    pub fn eval_string_type(&mut self,
                            _pieces:        &mut Vec<Piece<R>>,
                            _index:         &mut usize,
                            _data_offset:   u64,
                            _string_type:   &StringType
                            ) -> Result<Option<DebuggerValue<R>>>
    {
        unimplemented!();
    }


    pub fn eval_subrange_type(&mut self,
                              core:             &mut probe_rs::Core,
                              pieces:           &mut Vec<Piece<R>>,
                              index:            &mut usize,
                              data_offset:      u64,
                              subrange_type:    &SubrangeType
                              ) -> Result<Option<DebuggerValue<R>>>
    {
        match subrange_type.count {
            Some(val)   => return Ok(Some(DebuggerValue::Value(Value::U64(val)))),
            None        => (),
        };

        match &*subrange_type.r#type {
            Some(val)   => return self.eval_type(core, pieces, index, data_offset, val),
            None        => (),
        };

        Ok(None)
    }


    pub fn eval_generic_subrange_type(&mut self,
                                      _pieces:                  &mut Vec<Piece<R>>,
                                      _index:                   &mut usize,
                                      _data_offset:             u64,
                                      _generic_subrange_type:   &GenericSubrangeType
                                      ) -> Result<Option<DebuggerValue<R>>>
    {
        unimplemented!();
    }


    pub fn eval_template_type_parameter(&mut self,
                                        _pieces:                    &mut Vec<Piece<R>>,
                                        _index:                     &mut usize,
                                        _data_offset:               u64,
                                        _template_type_parameter:   &TemplateTypeParameter
                                        ) -> Result<Option<DebuggerValue<R>>>
    {
        Ok(None) // NOTE: I think that this is not used when evaluating the value of a type.
    }


    pub fn eval_variant_part(&mut self,
                             core:          &mut probe_rs::Core,
                             pieces:        &mut Vec<Piece<R>>,
                             index:         &mut usize,
                             data_offset:   u64,
                             variant_part:  &VariantPart
                             ) -> Result<Option<DebuggerValue<R>>>
    {
        match &variant_part.member {
            Some    (member)   => {
                let variant = get_udata(self.eval_member(core,
                                                         pieces,
                                                         index,
                                                         data_offset,
                                                         member)?.unwrap().to_value().unwrap()); // TODO: A more robust way of using the pieces.
                for v in &variant_part.variants {
                    if v.discr_value.unwrap() == variant {

                        return Ok(Some(DebuggerValue::Enum(Box::new(EnumValue{
                            name:   v.member.name.clone().unwrap(),
                            value:  self.eval_member(core, pieces, index, data_offset, &v.member)?.unwrap(),
                        }))));
                    }
                }
                unimplemented!();
            },
            None            => {
                unimplemented!();
            },
        };
    }


    pub fn eval_subroutine_type(&mut self,
                                _pieces:            &mut Vec<Piece<R>>,
                                _index:             &mut usize,
                                _data_offset:       u64,
                                _subroutine_type:   &SubroutineType
                                ) -> Result<Option<DebuggerValue<R>>>
    {
        unimplemented!();
    }


    pub fn eval_subprogram(&mut self,
                           _pieces:         &mut Vec<Piece<R>>,
                           _index:          &mut usize,
                           _data_offset:    u64,
                           _subprogram:     &Subprogram
                           ) -> Result<Option<DebuggerValue<R>>>
    {
        unimplemented!();
    }


    pub fn handle_eval_piece(&mut self,
                             core:              &mut probe_rs::Core,
                             pieces:            &mut Vec<Piece<R>>,
                             index:             &mut usize,
                             byte_size:         Option<u64>,
                             mut data_offset:   u64,
                             encoding:          Option<DwAte>
                             ) -> Result<Option<DebuggerValue<R>>>
    {
        if pieces.len() <= *index {
            return Ok(Some(DebuggerValue::OptimizedOut));
        }
        
        if pieces.len() > 1 {
            data_offset = 0;
        }
        
        let res = self.eval_piece(core,
                                  pieces[*index].clone(),
                                  byte_size,
                                  data_offset,
                                  encoding);

        match pieces[*index].size_in_bits {
            Some(val)   => {
                let bytes: i32 = match byte_size {
                    Some(val)   => (val*8) as i32,
                    None        => 32,
                };

                if (val as i32) - bytes < 1 {
                    pieces[*index].size_in_bits = Some(0);
                    *index += 1;
                } else {
                    pieces[*index].size_in_bits = Some(val - bytes as u64);
                }
            },
            None        => (),
        }

        return res;
    }
}

