use gimli::{
    DwAte,
    DwAddr,
};


pub trait TypeInfo {
    fn byte_size(&self) -> Option<u64>;
    fn alignment(&self) -> Option<u64>;
}


#[derive(Debug, PartialEq)]
pub enum DebuggerType {
    BaseType(BaseType),
    PointerType(PointerType),
    ArrayType(ArrayType),
    StructuredType(StructuredType),                 // TODO: Try to simplify the structure.
    UnionType(UnionType),                           // TODO: Try to simplify the structure.
    MemberType(MemberType),
    EnumerationType(EnumerationType),
//    Enumerator(Enumerator),
    StringType(StringType),                         // TODO: Parse all the important attributes. 
//    SubrangeType(SubrangeType),
    GenericSubrangeType(GenericSubrangeType),
    TemplateTypeParameter(TemplateTypeParameter),
    VariantPart(VariantPart),
//    Variant(Variant),
    SubroutineType(SubroutineType),
    Subprogram(Subprogram),
}
impl TypeInfo for DebuggerType {
    fn byte_size(&self) -> Option<u64> {
        match self {
            DebuggerType::BaseType(bt)                => bt.byte_size(),
            DebuggerType::PointerType(pt)             => pt.byte_size(),
            DebuggerType::ArrayType(at)               => at.byte_size(),
            DebuggerType::StructuredType(st)          => st.byte_size(),
            DebuggerType::UnionType(ut)               => ut.byte_size(),
            DebuggerType::MemberType(mt)              => mt.byte_size(),
            DebuggerType::EnumerationType(et)         => et.byte_size(),
            DebuggerType::StringType(st)              => st.byte_size(),
            DebuggerType::GenericSubrangeType(gt)     => gt.byte_size(),
            DebuggerType::TemplateTypeParameter(tp)   => tp.byte_size(),
            DebuggerType::VariantPart(vp)             => vp.byte_size(),
            DebuggerType::SubroutineType(st)          => st.byte_size(),
            DebuggerType::Subprogram(sp)              => sp.byte_size(),
        }
    }
    fn alignment(&self) -> Option<u64> {
        match self {
            DebuggerType::BaseType(bt)                => bt.alignment(),
            DebuggerType::PointerType(pt)             => pt.alignment(),
            DebuggerType::ArrayType(at)               => at.alignment(),
            DebuggerType::StructuredType(st)          => st.alignment(),
            DebuggerType::UnionType(ut)               => ut.alignment(),
            DebuggerType::MemberType(mt)              => mt.alignment(),
            DebuggerType::EnumerationType(et)         => et.alignment(),
            DebuggerType::StringType(st)              => st.alignment(),
            DebuggerType::GenericSubrangeType(gt)     => gt.alignment(),
            DebuggerType::TemplateTypeParameter(tp)   => tp.alignment(),
            DebuggerType::VariantPart(vp)             => vp.alignment(),
            DebuggerType::SubroutineType(st)          => st.alignment(),
            DebuggerType::Subprogram(sp)              => sp.alignment(),
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct BaseType {
    pub name:               Option<String>,
    pub encoding:           DwAte,
//    pub endianity: Option<>, //TODO
    pub byte_size:          Option<u64>,
    pub bit_size:           Option<u64>,
    pub data_bit_offset:    Option<u64>,
    pub alignment:          Option<u64>,
    // NOTE: May have more attributes.
}
impl TypeInfo for BaseType {
    fn byte_size(&self) -> Option<u64> {// TODO: use bit_size
        self.byte_size
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}

// pub struct UnspecifiedType {} // TODO: Don't know if this is used by rust.

// NOTE: Maybe combine all these into one modifier type.
//pub struct AtomicType {} // TODO: Don't know if these are used by rust.
//pub struct ConstType {}
//pub struct ImmutableType {}
//pub struct PackedType {}

#[derive(Debug, PartialEq)]
pub struct PointerType {
    pub address_class:  Option<DwAddr>,
    pub alignment:      Option<u64>,
    pub bit_size:       Option<u64>,
    pub byte_size:      Option<u64>,
    pub name:           Option<String>,
    pub r#type:         Box<DebuggerType>,
}
impl TypeInfo for PointerType {
    fn byte_size(&self) -> Option<u64> {// TODO: use bit_size
        match self.byte_size {
            Some(val)   => Some(val),
            None        => (*self.r#type).byte_size(),
        }
    }
    fn alignment(&self) -> Option<u64> {
        match self.alignment {
            Some(val)   => Some(val),
            None        => (*self.r#type).alignment(),
        }
    }
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
    pub alignment:      Option<u64>,
    pub bit_size:       Option<u64>,
    pub byte_size:      Option<u64>,
    pub name:           Option<String>,
    pub r#type:         Box<DebuggerType>,
    pub dimensions:     Vec<ArrayDimension>, // NOTE: Should be DW_TAG_subrange_type or DW_TAG_enumeration_type.
    // NOTE: Special case for array with dynamic rank, then the array dimensions are described by
    // one DW_TAG_generic_subrange. It has the same attribute as DW_TAG_subrange_type but there is
    // always only one. This case only happens when the DW_AT_rank attribute is present.
}
impl TypeInfo for ArrayType {
    fn byte_size(&self) -> Option<u64> {// TODO: use bit_size
        match self.byte_size {
            Some(val)   => Some(val),
            None        => (*self.r#type).byte_size(),
        }
    }
    fn alignment(&self) -> Option<u64> {
        match self.alignment {
            Some(val)   => Some(val),
            None        => (*self.r#type).alignment(),
        }
    }
}


#[derive(Debug, PartialEq)]
pub enum ArrayDimension {
    EnumerationType(EnumerationType),
    SubrangeType(SubrangeType),
}
impl TypeInfo for ArrayDimension {
    fn byte_size(&self) -> Option<u64> {
        match self {
            ArrayDimension::EnumerationType(et) => et.byte_size(),
            ArrayDimension::SubrangeType(st)    => st.byte_size(),
        }
    }
    fn alignment(&self) -> Option<u64> {
        match self {
            ArrayDimension::EnumerationType(et) => et.alignment(),
            ArrayDimension::SubrangeType(st)    => st.alignment(),
        }
    }
}


//pub struct CoArrays {} // TODO: Don't know if this is used by rust.


// NOTE: Maybe combine these three structured type into one.
// NOTE: There are a lot more attributes in the Dwarf spec, but most of them don't seam to be used
// by rust.
#[derive(Debug, PartialEq)]
pub struct StructuredType {
    pub alignment:  Option<u64>,
    pub bit_size:   Option<u64>,
    pub byte_size:  Option<u64>,
    pub children:   Vec<Box<DebuggerType>>,
    pub name:       Option<String>,
}
impl TypeInfo for StructuredType {
    fn byte_size(&self) -> Option<u64> {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}

// NOTE: There are a lot more attributes in the Dwarf spec, but most of them don't seam to be used
// by rust.
#[derive(Debug, PartialEq)]
pub struct UnionType {
    pub alignment:  Option<u64>,
    pub bit_size:   Option<u64>,
    pub byte_size:  Option<u64>,
    pub children:   Vec<Box<DebuggerType>>, // Maybe make this more specific so it is easier to parse the value later.
    pub name:       Option<String>,
}
impl TypeInfo for UnionType {
    fn byte_size(&self) -> Option<u64> {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}


#[derive(Debug, PartialEq)]
pub struct MemberType {
    pub accessibility:          Option<bool>,
    pub alignment:              Option<u64>, // NOTE: This is not pressent in Dwarf 5 spec, but is present in the debug info for rust.
    pub artificial:             Option<bool>,
    pub bit_size:               Option<u64>,
    pub byte_size:              Option<u64>,
    pub data_bit_offset:        Option<u64>,
    pub data_member_location:   Option<u64>,
    pub mutable:                Option<bool>,
    pub name:                   Option<String>,
    pub r#type:                 Box<DebuggerType>,
}
impl TypeInfo for MemberType {
    fn byte_size(&self) -> Option<u64> {// TODO: use bit_size.
        match self.byte_size {
            Some(val)   => Some(val),
            None        => (*self.r#type).byte_size(),
        }
    }
    fn alignment(&self) -> Option<u64> {
        match self.alignment {
            Some(val)   => Some(val),
            None        => (*self.r#type).alignment(),
        }
    }
}

//pub struct ClassType {} // TODO: Don't know if this is used by rust.


//pub struct ConditionEntries {} // TODO: Don't know if this is used by rust.


#[derive(Debug, PartialEq)]
pub struct EnumerationType {
    pub accessibility:  Option<bool>,
    pub alignment:      Option<u64>,
    pub bit_size:       Option<u64>,
    pub byte_size:      Option<u64>,
    //pub data_location:  Option<u64>,
    pub enum_class:     Option<bool>,
    pub enumerations:   Vec<Enumerator>,
    pub methods:        Vec<Subprogram>,    
    pub name:           Option<String>,
    pub r#type:         Box<Option<DebuggerType>>,
}
impl TypeInfo for EnumerationType {
    fn byte_size(&self) -> Option<u64> {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}


#[derive(Debug, PartialEq)]
pub struct Enumerator {
    pub name:           String,
    pub const_value:    u64,    // TODO: Can be any constant value,
}
impl TypeInfo for Enumerator {
    fn byte_size(&self) -> Option<u64> {
        None
    }
    fn alignment(&self) -> Option<u64> {
        None
    }
}


#[derive(Debug, PartialEq)]
pub struct StringType {
    pub accessibility:              Option<bool>,
    pub alignment:                  Option<u64>,
    pub bit_size:                   Option<u64>,
    pub byte_size:                  Option<u64>,
    pub name:                       Option<String>,
    pub string_length:              Option<u64>,
    pub string_length_bit_size:     Option<u64>,
    pub string_length_byte_size:    Option<u64>,
}
impl TypeInfo for StringType {
    fn byte_size(&self) -> Option<u64> {// TODO: bit_size
        self.byte_size
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}


//pub struct SetType {} // TODO: Don't know if this is used by rust.


#[derive(Debug, PartialEq)]
pub struct SubrangeType {
    pub accessibility:  Option<bool>,
    pub alignment:      Option<u64>,
    pub bit_size:       Option<u64>,
    pub byte_size:      Option<u64>,
    pub count:          Option<u64>,
    pub lower_bound:    Option<i64>,
    pub name:           Option<String>,
    pub r#type:         Box<Option<DebuggerType>>,
    pub upper_bound:    Option<i64>,
}
impl TypeInfo for SubrangeType {
    fn byte_size(&self) -> Option<u64> {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}


#[derive(Debug, PartialEq)]
pub struct GenericSubrangeType {
    pub accessibility:  Option<bool>,
    pub alignment:      Option<u64>,
    pub bit_size:       Option<u64>,
    pub byte_size:      Option<u64>,
    pub count:          Option<u64>,
    pub lower_bound:    Option<i64>,
    pub name:           Option<String>,
    pub r#type:         Box<Option<DebuggerType>>,
    pub upper_bound:    Option<i64>,
}
impl TypeInfo for GenericSubrangeType {
    fn byte_size(&self) -> Option<u64> {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}


//pub struct PtrToMemberType {} // TODO:
//
// TODO: Checkout File type, Dynamic Type, Template Alias Entries, Dynamic Properties of Types in
// the Dwarf spec.


#[derive(Debug, PartialEq)]
pub struct TemplateTypeParameter {
//    pub default_value:  Option<u64>,
    pub name:           Option<String>,
    pub r#type:         Box<DebuggerType>,
    // TODO: Check for more possible attribute in Dwarf spec.
}
impl TypeInfo for TemplateTypeParameter {
    fn byte_size(&self) -> Option<u64> {
        (*self.r#type).byte_size()
    }
    fn alignment(&self) -> Option<u64> {
        (*self.r#type).alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct VariantPart {
    pub accessibility:  Option<bool>,
    //pub discr:          Option<u64>, // TODO
    pub member:         Option<MemberType>,
    pub variants:       Vec<Variant>,
    // TODO: Check for more possible attribute in Dwarf spec.
}
impl TypeInfo for VariantPart {
    fn byte_size(&self) -> Option<u64> {
        unimplemented!();
    }
    fn alignment(&self) -> Option<u64> {
        unimplemented!();
    }
}


#[derive(Debug, PartialEq)]
pub struct Variant {
    pub accessibility:  Option<bool>,
//    pub discr_list:     Option<Vec<u64>>, // TODO
    pub discr_value:    Option<u64>,
    pub member:         MemberType,
    // TODO: Check for more possible attribute in Dwarf spec.
}
impl TypeInfo for Variant {
    fn byte_size(&self) -> Option<u64> {
        self.member.byte_size()
    }
    fn alignment(&self) -> Option<u64> {
        self.member.alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct SubroutineType {
    pub accessibility:  Option<bool>,
    pub address_class:  Option<DwAddr>,
    pub alignment:      Option<u64>,
    pub name:           Option<String>,
    pub linkage_name:   Option<String>,
    pub r#type:         Box<Option<DebuggerType>>,
    // TODO: Check for more possible attribute in Dwarf spec.
}
impl TypeInfo for SubroutineType {
    fn byte_size(&self) -> Option<u64> {
        unimplemented!();
    }
    fn alignment(&self) -> Option<u64> {
        self.alignment
    }
}


#[derive(Debug, PartialEq)]
pub struct Subprogram { // TODO: Fix this and the parser.
    pub name:               Option<String>,
    pub linkage_name:       Option<String>,
//    pub r#type:             Box<Option<DebuggerType>>, // NOTE: This can create a loop if it is
//    in a structure type.
    // TODO: Handle the children.
    // TODO: Check for more possible attribute in Dwarf spec.
}
impl TypeInfo for Subprogram {
    fn byte_size(&self) -> Option<u64> {
        unimplemented!();
    }
    fn alignment(&self) -> Option<u64> {
        unimplemented!();
    }
}

