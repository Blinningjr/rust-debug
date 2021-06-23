use std::fmt;

use gimli::{
    Reader,
};

#[derive(Debug, Clone)]
pub enum EvaluatorValue<R: Reader<Offset = usize>> {
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


impl<R: Reader<Offset = usize>> fmt::Display for EvaluatorValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            EvaluatorValue::Value           (val)   => val.fmt(f),
            EvaluatorValue::Bytes           (byt)   => write!(f, "{:?}", byt),
            EvaluatorValue::Array           (arr)   => arr.fmt(f),
            EvaluatorValue::Struct          (stu)   => stu.fmt(f),
            EvaluatorValue::Enum            (enu)   => enu.fmt(f),
            EvaluatorValue::Union           (uni)   => uni.fmt(f),
            EvaluatorValue::Member          (mem)   => mem.fmt(f),
            EvaluatorValue::Name            (nam)   => nam.fmt(f),
            EvaluatorValue::OutOfRange              => write!(f, "< OutOfRange >"),
            EvaluatorValue::OptimizedOut            => write!(f, "< OptimizedOut >"),
            EvaluatorValue::ZeroSize                => write!(f, "< ZeroSize >"),
        };
    }
}

impl<R: Reader<Offset = usize>> EvaluatorValue<R> {
    pub fn to_value(self) -> Option<BaseValue> {
        match self {
            EvaluatorValue::Value    (val)  => Some(val),
            EvaluatorValue::Member   (val)  => val.value.to_value(),
            EvaluatorValue::OptimizedOut    => Some(BaseValue::U32(0)), // TODO: Check if this is correct. Think gdb does this.
            _                               => panic!("{:#?}", self), // None,
        }
    }
}


pub fn get_udata(value: BaseValue) -> u64 {
    match value {
       BaseValue::U8        (v) => v as u64,
       BaseValue::U16       (v) => v as u64,
       BaseValue::U32       (v) => v as u64,
       BaseValue::U64       (v) => v,
       BaseValue::Generic   (v) => v,
       _                    => unimplemented!(),
    }
}

fn format_values<R: Reader<Offset = usize>>(values: &Vec<EvaluatorValue<R>>) -> String {
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



#[derive(Debug, Clone)]
pub struct ArrayValue<R: Reader<Offset = usize>> {
    pub values:  Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for ArrayValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ {} ]", format_values(&self.values))
    }
}

#[derive(Debug, Clone)]
pub struct StructValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for StructValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {{ {} }}", self.name, format_values(&self.members))
    }
}

#[derive(Debug, Clone)]
pub struct EnumValue<R: Reader<Offset = usize>> {
    pub name:   String,
    pub value: EvaluatorValue<R>,
}

impl<R: Reader<Offset = usize>> fmt::Display for EnumValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}", self.name, self.value)
    }
}

#[derive(Debug, Clone)]
pub struct UnionValue<R: Reader<Offset = usize>> {
    pub name:       String,
    pub members:    Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for UnionValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ( {} )", self.name, format_values(&self.members))
    }
}

#[derive(Debug, Clone)]
pub struct MemberValue<R: Reader<Offset = usize>> {
    pub name:   Option<String>,
    pub value:  EvaluatorValue<R>,
}

impl<R: Reader<Offset = usize>> fmt::Display for MemberValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match &self.name {
            Some(name)  => write!(f, "{}::{}", name, self.value),
            None        => write!(f, "{}", self.value),
        };
    }
}


#[derive(Debug, Clone)]
pub enum BaseValue {
    Generic(u64),

    Address32(u32),
    Bool(bool),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),

    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64), 

    F32(f32),
    F64(f64),
}

impl fmt::Display for BaseValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            BaseValue::Bool         (val)   => write!(f, "Bool {}", val),
            BaseValue::Generic      (val)   => write!(f, "Generic {}", val),
            BaseValue::I8           (val)   => write!(f, "I8 {}", val),
            BaseValue::U8           (val)   => write!(f, "U8 {}", val),
            BaseValue::I16          (val)   => write!(f, "I16 {}", val),
            BaseValue::U16          (val)   => write!(f, "U16 {}", val),
            BaseValue::I32          (val)   => write!(f, "I32 {}", val),
            BaseValue::U32          (val)   => write!(f, "U32 {}", val),
            BaseValue::I64          (val)   => write!(f, "I64 {}", val),
            BaseValue::U64          (val)   => write!(f, "U64 {}", val),
            BaseValue::F32          (val)   => write!(f, "F32 {}", val),
            BaseValue::F64          (val)   => write!(f, "F64 {}", val),
            BaseValue::Address32    (val)   => write!(f, "'Address' {:#10x}", val),
        };
    }
}



pub fn convert_to_gimli_value(value: BaseValue) -> gimli::Value {
    match value {
        BaseValue::Bool         (val)   => gimli::Value::Generic(match val { true => 1, false => 0,}),
        BaseValue::Generic      (val)   => gimli::Value::Generic(val),
        BaseValue::I8           (val)   => gimli::Value::I8(val),
        BaseValue::U8           (val)   => gimli::Value::U8(val),
        BaseValue::I16          (val)   => gimli::Value::I16(val),
        BaseValue::U16          (val)   => gimli::Value::U16(val),
        BaseValue::I32          (val)   => gimli::Value::I32(val),
        BaseValue::U32          (val)   => gimli::Value::U32(val),
        BaseValue::I64          (val)   => gimli::Value::I64(val),
        BaseValue::U64          (val)   => gimli::Value::U64(val),
        BaseValue::F32          (val)   => gimli::Value::F32(val),
        BaseValue::F64          (val)   => gimli::Value::F64(val),
        BaseValue::Address32    (val)   => gimli::Value::Generic(val as u64),
    }
}


pub fn convert_from_gimli_value(value: gimli::Value) -> BaseValue {
    match value {
        gimli::Value::Generic  (val)   => BaseValue::Generic(val),
        gimli::Value::I8       (val)   => BaseValue::I8(val),
        gimli::Value::U8       (val)   => BaseValue::U8(val),
        gimli::Value::I16      (val)   => BaseValue::I16(val),
        gimli::Value::U16      (val)   => BaseValue::U16(val),
        gimli::Value::I32      (val)   => BaseValue::I32(val),
        gimli::Value::U32      (val)   => BaseValue::U32(val),
        gimli::Value::I64      (val)   => BaseValue::I64(val),
        gimli::Value::U64      (val)   => BaseValue::U64(val),
        gimli::Value::F32      (val)   => BaseValue::F32(val),
        gimli::Value::F64      (val)   => BaseValue::F64(val),
    }
}


