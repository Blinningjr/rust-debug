use gimli::{
    Reader,
    Value,
};


use std::collections::HashMap;


#[derive(Debug)]
pub enum DebuggerValue<R: Reader<Offset = usize>> {
    Value(Value),
    Bytes(R),
    Raw(Vec<u32>),
    Struct(Box<StructValue<R>>),
    Enum(Box<EnumValue<R>>),
    Non,
}

#[derive(Debug)]
pub struct StructValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub attributes: HashMap<String, DebuggerValue<R>>,
}

#[derive(Debug)]
pub struct EnumValue<R: Reader<Offset = usize>> {
    pub name:   String,
    pub value:  u64,
    pub member: (String, DebuggerValue<R>),
}


impl<R: Reader<Offset = usize>> DebuggerValue<R> {
    pub fn to_value(self) -> Value {
        match self {
            DebuggerValue::Value(val)   => return val,
            _                           => unimplemented!(),
        };
    }
}

