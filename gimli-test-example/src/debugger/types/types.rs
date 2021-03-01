use gimli::{
    Reader,
    DwAte,
    DwAddr,
};


use std::collections::HashMap;


pub trait TypeInfo {
    fn byte_size(&self) -> u64;
    fn alignment(&self) -> Option<u64>;
}


#[derive(Debug, PartialEq)]
pub enum DebuggerType {
    Enum(Enum),
    EnumerationType(EnumerationType),
    Struct(Struct),
    BaseType(BaseType),
    Union(UnionType),
    Array(ArrayType),
    Pointer(PointerType),
//    TemplateParameter(TemplateParameter),
    Non,
}
impl TypeInfo for DebuggerType {
    fn byte_size(&self) -> u64 {
        match self {
            DebuggerType::Enum(e)               => e.byte_size(),
            DebuggerType::EnumerationType(et)   => et.byte_size(),
            DebuggerType::Struct(s)             => s.byte_size(),
            DebuggerType::BaseType(bt)          => bt.byte_size(),
            DebuggerType::Union(ut)             => ut.byte_size(),
            DebuggerType::Array(at)             => at.byte_size(),
            DebuggerType::Pointer(pt)           => pt.byte_size(),
            DebuggerType::Non                   => 0,
        }
    }
    fn alignment(&self) -> Option<u64>{
        match self {
            DebuggerType::Enum(e)               => e.alignment(),
            DebuggerType::EnumerationType(et)   => et.alignment(),
            DebuggerType::Struct(s)             => s.alignment(),
            DebuggerType::BaseType(bt)          => bt.alignment(),
            DebuggerType::Union(ut)             => ut.alignment(),
            DebuggerType::Array(at)             => at.alignment(),
            DebuggerType::Pointer(pt)           => pt.alignment(),
            DebuggerType::Non                   => None,
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct BaseType {
    pub name:       String,
    pub encoding:   DwAte,
    pub byte_size:  u64,
}

impl TypeInfo for BaseType {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        None
    }
}


#[derive(Debug, PartialEq)]
pub struct Struct {
    pub name:       String,
    pub byte_size:  u64,
    pub alignment:  u64,
    pub members:    Vec<Member>,
}

impl TypeInfo for Struct {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}


#[derive(Debug, PartialEq)]
pub struct Enum {
    pub name:       String,
    pub byte_size:  u64,
    pub alignment:  u64,
    pub index_type: ArtificialMember,
    pub variants:   HashMap<u64, Member>,
}
impl TypeInfo for Enum {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}

#[derive(Debug, PartialEq)]
pub struct EnumerationType {
    pub name:           String,
    pub enum_class:     bool,
    pub r#type:         Box<DebuggerType>,
    pub byte_size:      u64,
    pub alignment:      u64, 
    pub enumerators:    Vec<Enumerator>,
}
impl TypeInfo for EnumerationType {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}

#[derive(Debug, PartialEq)]
pub struct Enumerator {
    pub name:           String,
    pub const_value:    u64,
}
impl TypeInfo for Enumerator {
    fn byte_size(&self) -> u64 {
        1
    }
    fn alignment(&self) -> Option<u64>{
        None
    }
}


#[derive(Debug, PartialEq)]
pub struct Member {
    pub name:                   String,
    pub r#type:                 Box<DebuggerType>,
    pub alignment:              u64,
    pub data_member_location:   u64,
}
impl TypeInfo for Member {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}


#[derive(Debug, PartialEq)]
pub struct ArtificialMember {
    pub r#type:                 Box<DebuggerType>,
    pub alignment:              u64,
    pub data_member_location:   u64,
}
impl TypeInfo for ArtificialMember {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}


#[derive(Debug, PartialEq)]
pub struct TemplateParameter {
    pub name:   String,
    pub r#type: Box<DebuggerType>,
}
impl TypeInfo for TemplateParameter {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct UnionType {
    pub name:       String,
    pub byte_size:  u64,
    pub alignment:  u64,
    pub members:    Vec<Member>,
    pub tparams:    Vec<TemplateParameter>,
}
impl TypeInfo for UnionType {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}


#[derive(Debug, PartialEq)]
pub struct ArrayType {
    pub r#type: Box<DebuggerType>,
    pub range:  SubRangeType,
}
impl TypeInfo for ArrayType {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct SubRangeType {
    pub r#type:         Box<DebuggerType>,
    pub lower_bound:    i64,
    pub count:          u64,
}
impl TypeInfo for SubRangeType {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct PointerType {
    pub name:           String,
    pub r#type:         Box<DebuggerType>,
    pub address_class:  DwAddr,
}
impl TypeInfo for PointerType {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
    }
}
