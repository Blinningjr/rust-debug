use std::fmt;

use gimli::{
    Reader,
};



#[derive(Debug)]
pub(super) enum PartialValue<R: Reader<Offset = usize>> {
    Array(Box<PartialArrayValue<R>>),
    Struct(Box<PartialStructValue<R>>),
    Enum(Box<PartialEnumValue>),
    Union(Box<PartialUnionValue<R>>),
    NotEvaluated,
}

#[derive(Debug)]
pub(super) struct PartialArrayValue<R: Reader<Offset = usize>> {
    pub values:  Vec<EvaluatorValue<R>>,
}

#[derive(Debug)]
pub(super) struct PartialStructValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<EvaluatorValue<R>>,
}


#[derive(Debug)]
pub(super) struct PartialUnionValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<EvaluatorValue<R>>,
}


#[derive(Debug)]
pub(super) struct PartialEnumValue {
    pub name:       String,
    pub enum_val:   u32,
}


#[derive(Debug)]
pub(super) enum EvaluatorValue<R: Reader<Offset = usize>> {
    Value(BaseValue),
    Bytes(R),
    
    Array(Box<ArrayValue<R>>),
    Struct(Box<StructValue<R>>),
    Enum(Box<EnumValue<R>>),
    Union(Box<UnionValue<R>>),
    Member(Box<MemberValue<R>>),
    Name(String),

    OutOfRange,     // NOTE: Variable does not have a value currently.
    OptimizedOut,   // NOTE: Value is optimized out.
    ZeroSize, 
}


#[derive(Debug)]
pub(super) enum BaseValue {
    Generic(u64),

    Address32(u32),
//    Bool(bool),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),

    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64), 
}



// Old values

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

    OutOfRange,     // NOTE: Variable does not have a value currently.
    OptimizedOut,   // NOTE: Value is optimized out.
    ZeroSize, 
}

impl<R: Reader<Offset = usize>> fmt::Display for DebuggerValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            DebuggerValue::Value           (val)   => val.fmt(f),
            DebuggerValue::Bytes           (byt)   => write!(f, "{:?}", byt),
            DebuggerValue::Array           (arr)   => arr.fmt(f),
            DebuggerValue::Struct          (stu)   => stu.fmt(f),
            DebuggerValue::Enum            (enu)   => enu.fmt(f),
            DebuggerValue::Union           (uni)   => uni.fmt(f),
            DebuggerValue::Member          (mem)   => mem.fmt(f),
            DebuggerValue::Name            (nam)   => nam.fmt(f),
            DebuggerValue::OutOfRange              => write!(f, "< OutOfRange >"),
            DebuggerValue::OptimizedOut            => write!(f, "< OptimizedOut >"),
            DebuggerValue::ZeroSize                => write!(f, "< ZeroSize >"),
        };
    }
}


#[derive(Debug)]
pub enum Value {
    Generic(u64),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
    Address32(u32),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            Value::Generic      (val)   => write!(f, "Generic {}", val),
            Value::I8           (val)   => write!(f, "I8 {}", val),
            Value::U8           (val)   => write!(f, "U8 {}", val),
            Value::I16          (val)   => write!(f, "I16 {}", val),
            Value::U16          (val)   => write!(f, "U16 {}", val),
            Value::I32          (val)   => write!(f, "I32 {}", val),
            Value::U32          (val)   => write!(f, "U32 {}", val),
            Value::I64          (val)   => write!(f, "I64 {}", val),
            Value::U64          (val)   => write!(f, "U64 {}", val),
            Value::F32          (val)   => write!(f, "F32 {}", val),
            Value::F64          (val)   => write!(f, "F64 {}", val),
            Value::Address32    (val)   => write!(f, "'Address' {:#10x}", val),
        };
    }
}


#[derive(Debug)]
pub struct ArrayValue<R: Reader<Offset = usize>> {
    pub values:  Vec<DebuggerValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for ArrayValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ {} ]", format_values(&self.values))
    }
}


#[derive(Debug)]
pub struct MemberValue<R: Reader<Offset = usize>> {
    pub name:   Option<String>,
    pub value:  DebuggerValue<R>,
}

impl<R: Reader<Offset = usize>> fmt::Display for MemberValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match &self.name {
            Some(name)  => write!(f, "{}::{}", name, self.value),
            None        => write!(f, "{}", self.value),
        };
    }
}


#[derive(Debug)]
pub struct StructValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<DebuggerValue<R>>,
    //pub attributes: HashMap<String, DebuggerValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for StructValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {{ {} }}", self.name, format_values(&self.members))
    }
}


#[derive(Debug)]
pub struct UnionValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<DebuggerValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for UnionValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ( {} )", self.name, format_values(&self.members))
    }
}


#[derive(Debug)]
pub struct EnumValue<R: Reader<Offset = usize>> {
    pub name:   String,
    pub value: DebuggerValue<R>,
}

impl<R: Reader<Offset = usize>> fmt::Display for EnumValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}", self.name, self.value)
    }
}


impl<R: Reader<Offset = usize>> DebuggerValue<R> {
    pub fn to_value(self) -> Option<Value> {
        match self {
            DebuggerValue::Value    (val)   => Some(val),
            DebuggerValue::Member   (val)   => val.value.to_value(),
            DebuggerValue::OptimizedOut     => Some(Value::U32(0)), // TODO: Check if this is correct. Think gdb does this.
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


fn format_values<R: Reader<Offset = usize>>(values: &Vec<DebuggerValue<R>>) -> String {
    let len = values.len(); 
    if len == 0 {
        return "".to_string();
    } else if len == 1 {
        return format!("{}", values[0]);
    }

    let mut res = format!("{}", values[0]);
    for i in 1..len {
        res = format!("{}, {}", res, values[i]);
    }
    return res;
}


pub fn convert_to_gimli_value(value: Value) -> gimli::Value {
    match value {
        Value::Generic      (val)   => gimli::Value::Generic(val),
        Value::I8           (val)   => gimli::Value::I8(val),
        Value::U8           (val)   => gimli::Value::U8(val),
        Value::I16          (val)   => gimli::Value::I16(val),
        Value::U16          (val)   => gimli::Value::U16(val),
        Value::I32          (val)   => gimli::Value::I32(val),
        Value::U32          (val)   => gimli::Value::U32(val),
        Value::I64          (val)   => gimli::Value::I64(val),
        Value::U64          (val)   => gimli::Value::U64(val),
        Value::F32          (val)   => gimli::Value::F32(val),
        Value::F64          (val)   => gimli::Value::F64(val),
        Value::Address32    (val)   => gimli::Value::Generic(val as u64),
    }
}


pub fn convert_from_gimli_value(value: gimli::Value) -> Value {
    match value {
        gimli::Value::Generic  (val)   => Value::Generic(val),
        gimli::Value::I8       (val)   => Value::I8(val),
        gimli::Value::U8       (val)   => Value::U8(val),
        gimli::Value::I16      (val)   => Value::I16(val),
        gimli::Value::U16      (val)   => Value::U16(val),
        gimli::Value::I32      (val)   => Value::I32(val),
        gimli::Value::U32      (val)   => Value::U32(val),
        gimli::Value::I64      (val)   => Value::I64(val),
        gimli::Value::U64      (val)   => Value::U64(val),
        gimli::Value::F32      (val)   => Value::F32(val),
        gimli::Value::F64      (val)   => Value::F64(val),
    }
}


