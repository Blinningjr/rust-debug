use super::{
    Debugger,
    type_parser::{
        DebuggerType,
        Enum,
        Struct,
        BaseType,
        ByteSize,
        Member,
    },
    evaluate::{
        DebuggerValue,
        StructValue,
        EnumValue,
        eval_base_type,
    },
};


use gimli::{
    Reader,
    Value,
};


use std::collections::HashMap;


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn parse_value(&mut self,
                       data: Vec<u32>,
                       dtype: &DebuggerType
                       ) -> gimli::Result<DebuggerValue<R>>
    {
        match dtype {
            DebuggerType::Enum(e) => self.parse_enum_value(data, e),
            DebuggerType::Struct(s) => self.parse_struct_value(data, s),
            DebuggerType::BaseType(bt) => self.parse_base_type_value(data, bt),
            DebuggerType::Non => Ok(DebuggerValue::Non),
        }
    }

    
    pub fn parse_base_type_value(&mut self,
                                 mut data: Vec<u32>,
                                 btype: &BaseType
                                 ) -> gimli::Result<DebuggerValue<R>>
    {
        let value = eval_base_type(&data[..], btype.encoding, btype.byte_size);
        return Ok(DebuggerValue::Value(value));
    }


    pub fn parse_enum_value(&mut self,
                                 data: Vec<u32>,
                                 etype: &Enum
                                 ) -> gimli::Result<DebuggerValue<R>>
    {
        let name = etype.name.clone();
        let amem = &etype.index_type;
        let index_data = self.parse_data(data.clone(), amem.byte_size(), amem.data_member_location);
        let value: u64 = match self.parse_value(index_data, &(*amem.r#type))? {
            DebuggerValue::Value(val) => match val {
                Value::U8(v) => v as u64,
                Value::U16(v) => v as u64,
                Value::U32(v) => v as u64,
                Value::U64(v) => v,
                _ => panic!("Expected unsinged int"),
            },
            _ => panic!("Expected unsinged int"),
        };

        let (mname, member) = self.parse_member_value(data, etype.variants.get(&value).unwrap())?;

        return Ok(DebuggerValue::Enum(Box::new(EnumValue {
            name: name,
            value: value,
            member: (mname, member),
        })));
    }


    pub fn parse_struct_value(&mut self,
                                 data: Vec<u32>,
                                 stype: &Struct
                                 ) -> gimli::Result<DebuggerValue<R>>
    {
        let name = stype.name.clone();
        let mut attributes = HashMap::new();
        for member in &stype.members {
            let (vname, value) = self.parse_member_value(data.clone(), member)?;
            attributes.insert(vname, value);
        }
        return Ok(DebuggerValue::Struct(Box::new(StructValue {
            name: name,
            attributes: attributes,
        })));
    }


    pub fn parse_member_value(&mut self,
                              mut data: Vec<u32>,
                              member: &Member
                              ) -> gimli::Result<(String, DebuggerValue<R>)>
    {
        let name = member.name.clone();
        data = self.parse_data(data, member.byte_size(), member.data_member_location);
        let value = self.parse_value(data, &(*member.r#type))?;
        return Ok((name, value));
    }


    pub fn parse_data(&mut self,
                      mut data: Vec<u32>,
                      byte_size: u64,
                      data_member_location: u64
                      ) -> Vec<u32>
    {
        if (data.len() as u64) * 4 < byte_size + data_member_location {
            panic!("Somhting went very wrong");
        }
        for _ in 0..(data_member_location/4) {
            data.remove(0);
        }

        let first_offset: u64 = data_member_location%4; // In bytes
        //let first_mask = u32::MAX >> (first_offset as u32 * 8);

        //let last_offset: u64 = match first_offset {
        //    0 => (4 - (byte_size%4))%4,
        //    _ => (4 - ((byte_size - (4 - first_offset))%4))%4,
        //}; // In bytes
        //let last_mask = u32::MAX << (last_offset as u32 * 8);

        let data_len = (first_offset + byte_size + 4 - 1)/4;
        while data.len() as u64 > data_len {
            data.pop();
        }
        
        //let last = data.len() - 1;
        //data[0] = data[0] & first_mask;
        //data[last] = data[last] & last_mask;

        return data; // TODO: Should the data also be shifted so it is correctly aligned with the vec?
        // TODO: Improve by smartly removing the data that is not needed.
    }
}

