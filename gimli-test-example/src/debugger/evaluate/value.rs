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
    Member(Box<MemberValue<R>>),
    OptimizedOut,
    Name(String),
}

#[derive(Debug)]
pub struct MemberValue<R: Reader<Offset = usize>> {
    pub name:   String,
    pub value:  DebuggerValue<R>,
}

#[derive(Debug)]
pub struct StructValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<DebuggerValue<R>>,
    //pub attributes: HashMap<String, DebuggerValue<R>>,
}

#[derive(Debug)]
pub struct EnumValue<R: Reader<Offset = usize>> {
    pub name:   String,
    pub value: DebuggerValue<R>,
}


impl<R: Reader<Offset = usize>> DebuggerValue<R> {
    pub fn to_value(self) -> Option<Value> {
        match self {
            DebuggerValue::Value(val)   => Some(val),
            _                           => None,
        }
    }
}


pub fn get_udata(value: Value) -> u64 {
    match value {
       Value::U8        (v) => v as u64,
       Value::U16       (v) => v as u64,
       Value::U32       (v) => v as u64,
       Value::U64       (v) => v,
       Value::Generic   (v) => v,
       _                    => unimplemented!(),
    }
}

