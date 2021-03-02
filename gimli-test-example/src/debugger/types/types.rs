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
    Unimplemented,
    BaseType(BaseType),
    PointerType(PointerType),
    ArrayType(ArrayType),
    StructuredType(StructuredType),
    UnionType(UnionType),
    MemberType(MemberType),
    EnumerationType(EnumerationType),
    StringType(StringType), 
    SubrangeType(SubrangeType),
    GenericSubrangeType(GenericSubrangeType),
}
//impl TypeInfo for DebuggerType {
//    fn byte_size(&self) -> u64 {
//        match self {
//        }
//    }
//    fn alignment(&self) -> Option<u64>{
//        match self {
//        }
//    }
//}


#[derive(Debug, PartialEq)]
pub struct BaseType {
    pub name:               Option<String>,
    pub encoding:           DwAte,
//    pub endianity: Option<>, //TODO
    pub byte_size:          Option<u64>,
    pub bit_size:           Option<u64>,
    pub data_bit_offset:    Option<u64>,
    // NOTE: May have more attributes.
}

// pub struct UnspecifiedType {} // TODO: Don't know if this is used by rust.

// NOTE: Maybe combine all these into one modifier type.
//pub struct AtomicType {} // TODO: Don't know if these are used by rust.
//pub struct ConstType {}
//pub struct ImmutableType {}
//pub struct PackedType {}

#[derive(Debug, PartialEq)]
pub struct PointerType {
    pub name:           Option<String>,
    pub r#type:         Box<DebuggerType>,
    pub address_class:  Option<DwAddr>,
}

//pub struct ReferenceType {} // TODO: Don't know if these are used by rust
//pub struct RestrictType {}
//pub struct RValueReferanceType {}
//pub struct SharedType {}
//pub struct VolatileType {}


//#[derive(Debug, PartialEq)]
//pub struct TypeDef { // TODO: Don't know if this used by rust.
//    pub name:   String,
//    pub r#type: Option<Box<DebuggerType>>,
//}


#[derive(Debug, PartialEq)]
pub struct ArrayType {
    pub name:           Option<String>,
    pub r#type:         Box<DebuggerType>,
    pub children:       Vec<Box<DebuggerType>>, // NOTE: Should be DW_TAG_subrange_type or DW_TAG_enumeration_type.
    // NOTE: Special case for array with dynamic rank, then the array dimensions are described by
    // one DW_TAG_generic_subrange. It has the same attribute as DW_TAG_subrange_type but there is
    // always only one. This case only happens when the DW_AT_rank attribute is present.

    //pub ordering:       Option<u64>, // TODO: Check if any of these are used by rust.
    //pub byte_stride:    Option<u64>,
    //pub bit_stride:     Option<u64>,
    //pub byte_size:      Option<u64>,
    //pub bit_size:       Option<u64>,
    //pub rank:           Option<u64>,
    //pub allocated:      Option<bool>,
    //pub associated:     Option<bool>,
    //pub data_location:  Option<bool>,
}


//pub struct CoArrays {} // TODO: Don't know if this is used by rust.


// NOTE: Maybe combine these three structured type into one.
// NOTE: There are a lot more attributes in the Dwarf spec, but most of them don't seam to be used
// by rust.
#[derive(Debug, PartialEq)]
pub struct StructuredType {
    pub name:       Option<String>,
    pub byte_size:  Option<u64>,
    pub bit_size:   Option<u64>,
    pub alignment:  Option<u64>,
    pub children:   Vec<Box<DebuggerType>>, // Maybe make this more specific so it is easier to parse the value later.
}

// NOTE: There are a lot more attributes in the Dwarf spec, but most of them don't seam to be used
// by rust.
#[derive(Debug, PartialEq)]
pub struct UnionType {
    pub name:       Option<String>,
    pub byte_size:  Option<u64>,
    pub bit_size:   Option<u64>,
    pub alignment:  Option<u64>,
    pub children:   Vec<Box<DebuggerType>>, // Maybe make this more specific so it is easier to parse the value later.
}


#[derive(Debug, PartialEq)]
pub struct MemberType {
    pub name:                   Option<String>,
    pub r#type:                 Box<DebuggerType>,
    pub accessibility:          Option<bool>,
    pub mutable:                Option<bool>,
    pub data_member_location:   Option<u64>,
    pub data_bit_offset:        Option<u64>,
    pub byte_size:              Option<u64>,
    pub bit_size:               Option<u64>,
    pub alignment:              Option<u64>, 
}

//pub struct ClassType {} // TODO: Don't know if this is used by rust.


//pub struct ConditionEntries {} // TODO: Don't know if this is used by rust.


#[derive(Debug, PartialEq)]
pub struct EnumerationType {
    pub name:           Option<String>,
    pub r#type:         Box<Option<DebuggerType>>,
    pub byte_size:      Option<u64>,
    pub bit_size:       Option<u64>,
    pub alignment:      Option<u64>,
    pub enum_class:     Option<bool>,
    pub enumerations:   Vec<Enumerator>,

    // NOTE: Special case.
    //pub byte_stride:    Option<u64>,
    //pub bit_stride:     Option<u64>,
}


#[derive(Debug, PartialEq)]
pub struct Enumerator {
    pub name:           String,
    pub const_value:    u64,
}


//pub struct SubroutineType {} // TODO: Implement


#[derive(Debug, PartialEq)]
pub struct StringType {
    pub name:                       Option<String>,
    pub r#type:                     Box<Option<DebuggerType>>,
    pub byte_size:                  Option<u64>,
    pub bit_size:                   Option<u64>,
    pub alignment:                  Option<u64>,
    pub string_length:              Option<u64>,
    pub string_length_byte_size:    Option<u64>,
    pub string_length_bit_size:     Option<u64>,
}


//pub struct SetType {} // TODO: Don't know if this is used by rust.


#[derive(Debug, PartialEq)]
pub struct SubrangeType {
    pub name:           Option<String>,
    pub r#type:         Box<Option<DebuggerType>>,
    pub byte_size:      Option<u64>,
    pub bit_size:       Option<u64>,
    //pub threads_scaled: Option<bool>,
    pub lower_bound:    Option<i64>,
    pub upper_bound:    Option<i64>,
    pub count:          Option<u64>,
    //pub byte_stride:    Option<u64>,
    //pub bit_stride:     Option<u64>,
}


#[derive(Debug, PartialEq)]
pub struct GenericSubrangeType {
    pub name:           Option<String>,
    pub r#type:         Box<Option<DebuggerType>>,
    pub byte_size:      Option<u64>,
    pub bit_size:       Option<u64>,
    //pub threads_scaled: Option<bool>,
    pub lower_bound:    Option<i64>,
    pub upper_bound:    Option<i64>,
    pub count:          Option<u64>,
    //pub byte_stride:    Option<u64>,
    //pub bit_stride:     Option<u64>,
}


//pub struct PtrToMemberType {} // TODO:
//
// TODO: Checkout File type, Dynamic Type, Template Alias Entries, Dynamic Properties of Types in
// the Dwarf spec.


