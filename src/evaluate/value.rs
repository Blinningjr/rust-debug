use std::fmt;

use crate::evaluate::value_information::ValueInformation;
use crate::evaluate::value_information::ValuePiece;

use gimli::{
    Reader,
};

#[derive(Debug, Clone)]
pub enum EvaluatorValue<R: Reader<Offset = usize>> {
    Value(BaseValue, ValueInformation),
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
            EvaluatorValue::Value           (val, _)   => val.fmt(f),
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
            EvaluatorValue::Value    (val, _)  => Some(val),
            EvaluatorValue::Member   (val)  => val.value.to_value(),
            EvaluatorValue::OutOfRange      => None,
            EvaluatorValue::OptimizedOut    => None,
            EvaluatorValue::ZeroSize        => None,
            _                               => None, // TODO: Find a better solution then this.
        } }


    pub fn get_type(&self) -> String {
        match self {
            EvaluatorValue::Value   (val, _)    => val.get_type(),
            EvaluatorValue::Array   (arr)       => arr.get_type(),
            EvaluatorValue::Struct  (stu)       => stu.get_type(),
            EvaluatorValue::Enum    (enu)       => enu.get_type(),
            EvaluatorValue::Union   (uni)       => uni.get_type(),
            EvaluatorValue::Member  (mem)       => mem.get_type(),
            EvaluatorValue::Name    (nam)       => nam.to_string(),
            _                                   => "<unknown>".to_owned(),
        }
    }


    pub fn get_variable_information(self) -> Vec<ValueInformation> {
        match self {
            EvaluatorValue::Value (_, var_info) => vec!(var_info),
            EvaluatorValue::Array (arr) => {
                let mut info = vec!();
                for val in arr.values {
                    info.append(&mut val.get_variable_information());
                }
                info
            },
            EvaluatorValue::Struct (st) => {
                let mut info = vec!();
                for val in st.members {
                    info.append(&mut val.get_variable_information());
                }
                info
            },
            EvaluatorValue::Enum (en) => {
                en.value.get_variable_information()
            },
            EvaluatorValue::Union (un) => {
                let mut info = vec!();
                for val in un.members {
                    info.append(&mut val.get_variable_information());
                }
                info
            },
            EvaluatorValue::Member (me) => {
                me.value.get_variable_information()
            },
            EvaluatorValue::OptimizedOut => {
                vec!(ValueInformation::new(None, vec!(ValuePiece::Dwarf { value: None })))
            },
            EvaluatorValue::OutOfRange => {
                vec!(ValueInformation::new(None, vec!(ValuePiece::Dwarf { value: None })))
            },
            _ => vec!(),
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


fn format_types<R: Reader<Offset = usize>>(values: &Vec<EvaluatorValue<R>>) -> String {
    let len = values.len(); 
    if len == 0 {
        return "".to_string();
    } else if len == 1 {
        return format!("{}", values[0].get_type());
    }

    let mut res = format!("{}", values[0].get_type());
    for i in 1..len {
        res = format!("{}, {}", res, values[i].get_type());
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

impl<R: Reader<Offset = usize>> ArrayValue<R> {
    pub fn get_type(&self) -> String {
        format!("[ {} ]", format_types(&self.values))
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

impl<R: Reader<Offset = usize>> StructValue<R> {
    pub fn get_type(&self) -> String {
        format!("{} {{ {} }}", self.name, format_types(&self.members))
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

impl<R: Reader<Offset = usize>> EnumValue<R> {
    pub fn get_type(&self) -> String {
        format!("{}::{}", self.name, self.value.get_type())
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

impl<R: Reader<Offset = usize>> UnionValue<R> {
    pub fn get_type(&self) -> String {
        format!("{} ( {} )", self.name, format_types(&self.members))
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

impl<R: Reader<Offset = usize>> MemberValue<R> {
    pub fn get_type(&self) -> String {
        match &self.name {
            Some(name)  => format!("{}::{}", name, self.value.get_type()),
            None        => format!("{}", self.value.get_type()),
        }
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
            BaseValue::Bool         (val)   => write!(f, "{}", val),
            BaseValue::Generic      (val)   => write!(f, "{}", val),
            BaseValue::I8           (val)   => write!(f, "{}", val),
            BaseValue::U8           (val)   => write!(f, "{}", val),
            BaseValue::I16          (val)   => write!(f, "{}", val),
            BaseValue::U16          (val)   => write!(f, "{}", val),
            BaseValue::I32          (val)   => write!(f, "{}", val),
            BaseValue::U32          (val)   => write!(f, "{}", val),
            BaseValue::I64          (val)   => write!(f, "{}", val),
            BaseValue::U64          (val)   => write!(f, "{}", val),
            BaseValue::F32          (val)   => write!(f, "{}", val),
            BaseValue::F64          (val)   => write!(f, "{}", val),
            BaseValue::Address32    (val)   => write!(f, "'Address' {:#10x}", val),
        };
    }
}

impl BaseValue {
    pub fn get_type(&self) -> String {
        match self {
            BaseValue::Bool         (_)   => "bool".to_owned(),
            BaseValue::Generic      (_)   => "<unknown>".to_owned(),
            BaseValue::I8           (_)   => "i8".to_owned(),
            BaseValue::U8           (_)   => "u8".to_owned(),
            BaseValue::I16          (_)   => "i16".to_owned(),
            BaseValue::U16          (_)   => "u16".to_owned(),
            BaseValue::I32          (_)   => "i32".to_owned(),
            BaseValue::U32          (_)   => "u32".to_owned(),
            BaseValue::I64          (_)   => "i64".to_owned(),
            BaseValue::U64          (_)   => "u64".to_owned(),
            BaseValue::F32          (_)   => "f32".to_owned(),
            BaseValue::F64          (_)   => "f63".to_owned(),
            BaseValue::Address32    (_)   => "<32 bit address>".to_owned(),
        }
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

