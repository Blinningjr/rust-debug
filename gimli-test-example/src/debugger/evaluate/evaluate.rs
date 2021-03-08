use super::{
    Reader,
    Debugger,
    DebuggerValue,
    value::get_udata,
    eval_base_type,
    EnumValue,
    StructValue,
    MemberValue,
};


use crate::debugger::types::types::{
    DebuggerType,
    BaseType,
    MemberType,
    StructuredType,
    VariantPart,
    Variant,
};


use gimli::{
    Value,
    Result,
    Piece,
    Location,
    DwAte,
};


use probe_rs::MemoryInterface;


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn eval_type(&mut self, pieces: &mut Vec<Piece<R>>, dtype: &DebuggerType) -> Option<DebuggerValue<R>>
    {
        return match dtype {
            DebuggerType::BaseType              (bt)    => self.eval_basetype(pieces, bt),
            DebuggerType::PointerType           (pt)    => self.eval_pointer_type(),
            DebuggerType::ArrayType             (at)    => self.eval_array_type(),
            DebuggerType::StructuredType        (st)    => self.eval_structured_type(pieces, st),
            DebuggerType::UnionType             (ut)    => self.eval_union_type(),
            DebuggerType::MemberType            (mt)    => self.eval_member(pieces, mt),
            DebuggerType::EnumerationType       (et)    => self.eval_enumeration_type(),
            DebuggerType::StringType            (st)    => self.eval_string_type(),
            DebuggerType::GenericSubrangeType   (gt)    => self.eval_generic_subrange_type(),
            DebuggerType::TemplateTypeParameter (tp)    => self.eval_template_type_parameter(),
            DebuggerType::VariantPart           (vp)    => self.eval_variant_part(pieces, vp),
            DebuggerType::SubroutineType        (st)    => self.eval_subroutine_type(),
            DebuggerType::Subprogram            (sp)    => self.eval_subprogram(),
        };
    }


    pub fn eval_piece(&mut self, piece: Piece<R>, byte_size: Option<u64>, encoding: Option<DwAte>) -> Option<DebuggerValue<R>>
    {
        println!("{:#?}", piece);

        return match piece.location {
            Location::Empty                                   => Some(DebuggerValue::OptimizedOut),
            Location::Register        { register }            => self.eval_register(register),
            Location::Address         { address }             => self.eval_address(address, byte_size, encoding.unwrap()),
            Location::Value           { value }               => Some(DebuggerValue::Value(value)),
            Location::Bytes           { value }               => Some(DebuggerValue::Bytes(value)),
            Location::ImplicitPointer { value, byte_offset }  => unimplemented!(),
        };
    }


    pub fn eval_register(&mut self, register: gimli::Register) -> Option<DebuggerValue<R>>
    {
        let data = self.core.read_core_reg(register.0).unwrap();
        return Some(DebuggerValue::Value(Value::U32(data))); // TODO: Mask the important bits?
    }


    pub fn eval_address(&mut self, address: u64, byte_size: Option<u64>, encoding: DwAte) -> Option<DebuggerValue<R>>
    {
        let num_words = match byte_size {
            Some(val)   => (val + 4 - 1 )/4,
            None        => 1,
        };

        let mut data: Vec<u32> = vec![0; num_words as usize];
        self.core.read_32(address as u32, &mut data).unwrap();

        let mut res: Vec<u32> = Vec::new();
        for d in data.iter() {
            res.push(*d);
        }

        return Some(DebuggerValue::Value(eval_base_type(&data, encoding, byte_size.unwrap())));
        //return Some(DebuggerValue::Raw(res));
    }


    pub fn eval_basetype(&mut self, mut pieces: &mut Vec<Piece<R>>, base_type: &BaseType) -> Option<DebuggerValue<R>>
    {
        if pieces.len() > 0 {
            return self.eval_piece(pieces.remove(0), base_type.byte_size, Some(base_type.encoding));
        }
        return Some(DebuggerValue::OptimizedOut);
    }


    pub fn eval_pointer_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_array_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }


    pub fn eval_structured_type(&mut self, pieces: &mut Vec<Piece<R>>, structured_type: &StructuredType) -> Option<DebuggerValue<R>>
    {
        let mut members = Vec::new();
        for c in &structured_type.children {
            match &(**c) {
                DebuggerType::VariantPart   (vp)    => {
                    let members = vec!(self.eval_variant_part(pieces, &vp).unwrap());
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
        let members = members.into_iter().map(|m| self.eval_member(pieces, m).unwrap()).collect();

        return Some(DebuggerValue::Struct(Box::new(StructValue{
            name:       structured_type.name.clone().unwrap(),
            members:    members,
        })));
    }


    pub fn eval_union_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_member(&mut self, pieces: &mut Vec<Piece<R>>, member: &MemberType) -> Option<DebuggerValue<R>>
    {
        return Some(DebuggerValue::Member(Box::new(MemberValue{
            name:   member.name.clone().unwrap(),
            value:  self.eval_type(pieces, &member.r#type).unwrap(),
        })));
    }

    pub fn eval_enumeration_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_enumerator(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_string_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_subrange_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_generic_subrange_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_template_type_parameter(&mut self) -> Option<DebuggerValue<R>>
    {
        None // NOTE: I think that this is not used when evaluating the value of a type.
    }


    pub fn eval_variant_part(&mut self, mut pieces: &mut Vec<Piece<R>>, variant_part: &VariantPart) -> Option<DebuggerValue<R>>
    {
        match &variant_part.member {
            Some    (member)   => {
                let variant = get_udata(self.eval_member(pieces, member).unwrap().to_value().unwrap()); // TODO: A more robust way of using the pieces.
                for v in &variant_part.variants {
                    if v.discr_value.unwrap() == variant {
                        return Some(DebuggerValue::Enum(Box::new(EnumValue{
                            name:   v.member.name.clone().unwrap(),
                            value:  self.eval_member(pieces, &v.member).unwrap(),
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


    pub fn eval_subroutine_type(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }

    pub fn eval_subprogram(&mut self) -> Option<DebuggerValue<R>>
    {
        unimplemented!();
    }
}

