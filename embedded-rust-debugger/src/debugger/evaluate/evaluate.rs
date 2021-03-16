use super::{
    Debugger,
    DebuggerValue,
    eval_base_type,
    EnumValue,
    value::get_udata,
    value::StructValue,
    value::MemberValue,
    value::UnionValue,
    value::ArrayValue,
    value::Value,
    value::convert_from_gimli_value,
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


use probe_rs::MemoryInterface;


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn eval_type(&mut self,
                     pieces: &Vec<Piece<R>>,
                     index: &mut usize,
                     dtype: &DebuggerType
                     )-> Option<DebuggerValue<R>>
    {
        return match dtype {
            DebuggerType::BaseType              (bt)    => self.eval_basetype(pieces, index, bt),
            DebuggerType::PointerType           (pt)    => self.eval_pointer_type(pieces, index, pt),
            DebuggerType::ArrayType             (at)    => self.eval_array_type(pieces, index, at),
            DebuggerType::StructuredType        (st)    => self.eval_structured_type(pieces, index, st),
            DebuggerType::UnionType             (ut)    => self.eval_union_type(pieces, index, ut),
            DebuggerType::MemberType            (mt)    => self.eval_member(pieces, index, mt),
            DebuggerType::EnumerationType       (et)    => self.eval_enumeration_type(pieces, index, et),
            DebuggerType::StringType            (st)    => self.eval_string_type(pieces, index, st),
            DebuggerType::GenericSubrangeType   (gt)    => self.eval_generic_subrange_type(pieces, index, gt),
            DebuggerType::TemplateTypeParameter (tp)    => self.eval_template_type_parameter(pieces, index, tp),
            DebuggerType::VariantPart           (vp)    => self.eval_variant_part(pieces, index, vp),
            DebuggerType::SubroutineType        (st)    => self.eval_subroutine_type(pieces, index, st),
            DebuggerType::Subprogram            (sp)    => self.eval_subprogram(pieces, index, sp),
        };
    }


    pub fn eval_piece(&mut self,
                      piece: Piece<R>,
                      byte_size: Option<u64>,
                      encoding: Option<DwAte>
                      ) -> Option<DebuggerValue<R>>
    {
        //println!("{:#?}", piece);

        return match piece.location {
            Location::Empty                                   => Some(DebuggerValue::OptimizedOut),
            Location::Register        { register }            => self.eval_register(register),
            Location::Address         { address }             => self.eval_address(address, byte_size, encoding.unwrap()),
            Location::Value           { value }               => Some(DebuggerValue::Value(convert_from_gimli_value(value))),
            Location::Bytes           { value }               => Some(DebuggerValue::Bytes(value)),
            Location::ImplicitPointer { value: _, byte_offset: _ }  => unimplemented!(),
        };
    }


    pub fn eval_register(&mut self,
                         register: gimli::Register
                         ) -> Option<DebuggerValue<R>>
    {
        let data = self.core.read_core_reg(register.0).unwrap();
        return Some(DebuggerValue::Value(Value::U32(data))); // TODO: Mask the important bits?
    }


    pub fn eval_address(&mut self,
                        mut address: u64,
                        byte_size: Option<u64>,
                        encoding: DwAte
                        ) -> Option<DebuggerValue<R>>
    {
        let num_words = match byte_size {
            Some(val)   => (val + 4 - 1 )/4,
            None        => 1,
        };

        address -= address%4; // TODO: Is this correct?

        let mut data: Vec<u32> = vec![0; num_words as usize];
        self.core.read_32(address as u32, &mut data).unwrap();

        let mut res: Vec<u32> = Vec::new();
        for d in data.iter() {
            res.push(*d);
        }

        return Some(DebuggerValue::Value(eval_base_type(&data,
                                                        encoding,
                                                        byte_size.unwrap())));
    }


    pub fn eval_basetype(&mut self,
                         pieces: &Vec<Piece<R>>,
                         index: &mut usize,
                         base_type: &BaseType
                         ) -> Option<DebuggerValue<R>>
    {
        match base_type.byte_size {
            Some(0) => return Some(DebuggerValue::ZeroSize),
            _       => (),
        };

        let res = self.eval_piece(pieces[*index].clone(),
                                  base_type.byte_size,
                                  Some(base_type.encoding));
        if *index < pieces.len() - 1 {
            *index += 1;
        }

        return res;
    }


    pub fn eval_pointer_type(&mut self,
                             pieces: &Vec<Piece<R>>,
                             index: &mut usize,
                             pointer_type: &PointerType,
                             ) -> Option<DebuggerValue<R>>
    {
        return self.eval_type(pieces, index, &(*pointer_type.r#type));
    }


    pub fn eval_array_type(&mut self,
                           pieces: &Vec<Piece<R>>,
                           index: &mut usize,
                           array_type: &ArrayType
                           ) -> Option<DebuggerValue<R>>
    {
        let count = get_udata(match &array_type.dimensions[0] {
            ArrayDimension::EnumerationType (et)    => self.eval_enumeration_type(pieces, index, et),
            ArrayDimension::SubrangeType    (st)    => self.eval_subrange_type(pieces, index, st),
        }.unwrap().to_value().unwrap());

        let mut values = Vec::new();
        for _i in 0..count {
            values.push(self.eval_type(pieces,
                                       index,
                                       &array_type.r#type).unwrap());  // TODO: Fix so that it can read multiple of the same type.
        }
        
        return Some(DebuggerValue::Array(Box::new(ArrayValue{
            values: values,
        })));
    }


    pub fn eval_structured_type(&mut self,
                                pieces: &Vec<Piece<R>>,
                                index: &mut usize,
                                structured_type: &StructuredType
                                ) -> Option<DebuggerValue<R>>
    {
        let mut members = Vec::new();
        for c in &structured_type.children {
            match &(**c) {
                DebuggerType::VariantPart   (vp)    => {
                    let members = vec!(self.eval_variant_part(pieces, index, &vp).unwrap());

                    return Some(DebuggerValue::Struct(Box::new(StructValue{
                        name:       structured_type.name.clone().unwrap(),
                        members:    members,
                    })));
                },

                DebuggerType::MemberType    (mt)    => {
                    members.push(mt);
                },
                _ => continue,
            };
        }

        members.sort_by_key(|m| m.data_member_location);
        let members = members.into_iter().map(|m| self.eval_member(pieces, index, m).unwrap()).collect();

        return Some(DebuggerValue::Struct(Box::new(StructValue{
            name:       structured_type.name.clone().unwrap(),
            members:    members,
        })));
    }


    pub fn eval_union_type(&mut self,
                           pieces: &Vec<Piece<R>>,
                           index: &mut usize,
                           union_type: &UnionType
                           ) -> Option<DebuggerValue<R>>
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
        let members = members.into_iter().map(|m| self.eval_member(pieces, index, m).unwrap()).collect();

        return Some(DebuggerValue::Union(Box::new(UnionValue{
            name:       union_type.name.clone().unwrap(),
            members:    members,
        })));
    }


    pub fn eval_member(&mut self,
                       pieces: &Vec<Piece<R>>,
                       index: &mut usize,
                       member: &MemberType
                       ) -> Option<DebuggerValue<R>>
    {
        return Some(DebuggerValue::Member(Box::new(MemberValue{
            name:   member.name.clone(),
            value:  self.eval_type(pieces, index, &member.r#type).unwrap(),
        })));
    }


    pub fn eval_enumeration_type(&mut self,
                                 pieces: &Vec<Piece<R>>,
                                 index: &mut usize,
                                 enumeration_type: &EnumerationType
                                 ) -> Option<DebuggerValue<R>>
    {
        let value = get_udata(self.eval_type(pieces,
                                             index,
                                             (*enumeration_type.r#type).as_ref().unwrap()).unwrap().to_value().unwrap());

        for e in &enumeration_type.enumerations {
            if e.const_value == value {
                return Some(DebuggerValue::Enum(Box::new(EnumValue{
                    name:   enumeration_type.name.clone().unwrap(),
                    value:  DebuggerValue::Name(e.name.clone()),
                })));
            }
        }
        None
    }


    pub fn eval_enumerator(&mut self,
                           _pieces: &Vec<Piece<R>>,
                           _index: &mut usize,
                           _enumerator:  &Enumerator
                           ) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }


    pub fn eval_string_type(&mut self,
                            _pieces:        &Vec<Piece<R>>,
                            _index:         &mut usize,
                            _string_type:    &StringType
                            ) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }


    pub fn eval_subrange_type(&mut self,
                              pieces: &Vec<Piece<R>>,
                              index: &mut usize,
                              subrange_type: &SubrangeType
                              ) -> Option<DebuggerValue<R>>
    {
        match subrange_type.count {
            Some(val)   => return Some(DebuggerValue::Value(Value::U64(val))),
            None        => (),
        };

        match &*subrange_type.r#type {
            Some(val)   => return self.eval_type(pieces, index, val),
            None        => (),
        };
        None
    }


    pub fn eval_generic_subrange_type(&mut self,
                                      _pieces:                  &Vec<Piece<R>>,
                                      _index:                   &mut usize,
                                      _generic_subrange_type:   &GenericSubrangeType
                                      ) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }


    pub fn eval_template_type_parameter(&mut self,
                                        _pieces:                    &Vec<Piece<R>>,
                                        _index:                     &mut usize,
                                        _template_type_parameter:   &TemplateTypeParameter
                                        ) -> Option<DebuggerValue<R>>
    {
        None // NOTE: I think that this is not used when evaluating the value of a type.
    }


    pub fn eval_variant_part(&mut self,
                             pieces: &Vec<Piece<R>>,
                             index: &mut usize,
                             variant_part: &VariantPart
                             ) -> Option<DebuggerValue<R>>
    {
        match &variant_part.member {
            Some    (member)   => {
                let variant = get_udata(self.eval_member(pieces,
                                                         index,
                                                         member).unwrap().to_value().unwrap()); // TODO: A more robust way of using the pieces.
                for v in &variant_part.variants {
                    if v.discr_value.unwrap() == variant {

                        return Some(DebuggerValue::Enum(Box::new(EnumValue{
                            name:   v.member.name.clone().unwrap(),
                            value:  self.eval_member(pieces, index, &v.member).unwrap(),
                        })));
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
                                _pieces:            &Vec<Piece<R>>,
                                _index:             &mut usize,
                                _subroutine_type:   &SubroutineType
                                ) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }


    pub fn eval_subprogram(&mut self,
                           _pieces:     &Vec<Piece<R>>,
                           _index:      &mut usize,
                           _subprogram: &Subprogram
                           ) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }
}

