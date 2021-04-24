use super::{
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
    Result,
};


use probe_rs::MemoryInterface;



struct Evaluator<R: Reader<Offset = usize>> {
    stack: Vec<EvaluatorState<R>>,
    result: Option<super::value::EvaluatorValue<R>>,
    // TODO: Add hashmap for registers maybe?
}


// TODO: piece evaluator state.
struct EvaluatorState<R: Reader<Offset = usize>> {
    unit_offset: gimli::UnitSectionOffset,
    die_offset: gimli::UnitOffset,
    patrial_value: super::value::PartialValue<R>,
}

enum EvaluatorResult {
    Complete,
    RequireReg(u32),
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


    pub fn eval_enumerator(&mut self,
                           _pieces:         &mut Vec<Piece<R>>,
                           _index:          &mut usize,
                           _data_offset:    u64,
                           _enumerator:     &Enumerator
                           ) -> Result<Option<DebuggerValue<R>>>
    {
        unimplemented!();
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

