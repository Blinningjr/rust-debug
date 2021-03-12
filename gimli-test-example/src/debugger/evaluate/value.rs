use gimli::{
    Reader,
    Value,
};


#[derive(Debug)]
pub enum DebuggerValue<R: Reader<Offset = usize>> {
    Value(Value),
    Bytes(R),
//    Raw(Vec<u32>),
    Array(Box<ArrayValue<R>>),
    Struct(Box<StructValue<R>>),
    Enum(Box<EnumValue<R>>),
    Union(Box<UnionValue<R>>),
    Member(Box<MemberValue<R>>),
    Name(String),
    OptimizedOut,
    ZeroSize, 
}


#[derive(Debug)]
pub struct ArrayValue<R: Reader<Offset = usize>> {
    pub values:  Vec<DebuggerValue<R>>,
}

#[derive(Debug)]
pub struct MemberValue<R: Reader<Offset = usize>> {
    pub name:   Option<String>,
    pub value:  DebuggerValue<R>,
}

#[derive(Debug)]
pub struct StructValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<DebuggerValue<R>>,
    //pub attributes: HashMap<String, DebuggerValue<R>>,
}

#[derive(Debug)]
pub struct UnionValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<DebuggerValue<R>>,
}

#[derive(Debug)]
pub struct EnumValue<R: Reader<Offset = usize>> {
    pub name:   String,
    pub value: DebuggerValue<R>,
}


impl<R: Reader<Offset = usize>> DebuggerValue<R> {
    pub fn to_value(self) -> Option<Value> {
        match self {
            DebuggerValue::Value    (val)   => Some(val),
            DebuggerValue::Member   (val)   => val.value.to_value(),
            DebuggerValue::OptimizedOut     => Some(gimli::Value::U32(0)), // TODO: Check if this is correct. Think gdb does this.
            _                               => panic!("{:#?}", self), // None,
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

