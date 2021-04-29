use gimli::{
    Reader,
    DebuggingInformationEntry,
    AttributeValue::{
        AddressClass,
        DebugStrRef,
        Encoding,
        Data1,
        Data2,
        Data4,
        Data8,
        Udata,
        Sdata,
        Flag,
    },
    DwAddr,
    DwAte,
    Unit,
};


pub fn name_attribute<R: Reader<Offset = usize>>(dwarf: &gimli::Dwarf<R>,
                      die: &DebuggingInformationEntry<R>
                      ) -> Option<String>
{
    return match die.attr_value(gimli::DW_AT_name).ok()? {
        Some(DebugStrRef(offset)) => Some(dwarf.string(offset).ok()?.to_string().ok()?.to_string()),
        Some(unknown) => {
            println!("name_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        None => Some("<Unknown>".to_string()), // TODO: return None
    };
}


pub fn byte_size_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_byte_size).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("byte_size_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn bit_size_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_bit_size).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("bit_size_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn data_bit_offset_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_data_bit_offset).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("data_bit_offset_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn alignment_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_alignment).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("alignment_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn data_member_location_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_data_member_location).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("data_member_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn type_attribute<R: Reader<Offset = usize>>(dwarf:    &gimli::Dwarf<R>,
                      unit:      &Unit<R>,
                      die: &DebuggingInformationEntry<R>
                      ) -> Option<(gimli::UnitSectionOffset, gimli::UnitOffset)>
{
    match die.attr_value(gimli::DW_AT_type).ok()? {
        Some(gimli::AttributeValue::UnitRef(offset)) => {
            return Some((unit.header.offset(), offset));
        },
        Some(gimli::AttributeValue::DebugInfoRef(di_offset)) => {
            let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
            let mut iter = dwarf.debug_info.units();
            while let Ok(Some(header)) = iter.next() {
                let unit = dwarf.unit(header).unwrap();
                if let Some(offset) = offset.to_unit_offset(&unit) {
                    return Some((unit.header.offset(), offset));
                }
            }
            return None;
        },
        _ => return None,
    };
}


pub fn address_class_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<DwAddr>
{
    return match die.attr_value(gimli::DW_AT_address_class).ok()? {
        Some(AddressClass(val)) => Some(val),
        Some(unknown) => {
            println!("address_class_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn const_value_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_const_value).ok()? {
        Some(Udata(val)) => Some(val),
        Some(Sdata(val)) => Some(val as u64), // TODO: Should not be converted to unsigned
        Some(unknown) => {
            println!("const_class_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn enum_class_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<bool>
{
    return match die.attr_value(gimli::DW_AT_enum_class).ok()? {
        Some(Flag(val)) => Some(val),
        Some(unknown) => {
            println!("enum_class_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn lower_bound_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<i64>
{
    return match die.attr_value(gimli::DW_AT_lower_bound).ok()? {
        Some(Sdata(val)) => Some(val),
        Some(unknown) => {
            println!("lower_bound_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn upper_bound_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<i64>
{
    return match die.attr_value(gimli::DW_AT_upper_bound).ok()? {
        Some(Sdata(val)) => Some(val),
        Some(unknown) => {
            println!("upper_bound_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn count_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_count).ok()? {
        Some(Udata(val)) => Some(val),
        Some(Data1(val)) => Some(val as u64),
        Some(Data2(val)) => Some(val as u64),
        Some(Data4(val)) => Some(val as u64),
        Some(Data8(val)) => Some(val),
        Some(unknown) => {
            println!("count_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn encoding_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<DwAte>
{
    return match die.attr_value(gimli::DW_AT_encoding).ok()? {
        Some(Encoding(val)) => Some(val),
        Some(unknown) => {
            println!("encoding_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn accessibility_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<bool>
{
    return match die.attr_value(gimli::DW_AT_accessibility).ok()? {
        Some(Flag(val)) => Some(val),
        Some(unknown) => {
            println!("acessibility_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn mutable_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<bool>
{
    return match die.attr_value(gimli::DW_AT_mutable).ok()? {
        Some(Flag(val)) => Some(val),
        Some(unknown) => {
            println!("mutable_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn string_length_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_string_length).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("string_length_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn string_length_byte_size_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_string_length_byte_size).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("string_length_byte_size_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn string_length_bit_size_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_string_length_bit_size).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("string_length_bit_size_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn linkage_name_attribute<R: Reader<Offset = usize>>(dwarf: &gimli::Dwarf<R>,
                              die: &DebuggingInformationEntry<R>
                              ) -> Option<String>
{
    return match die.attr_value(gimli::DW_AT_linkage_name).ok()? {
        Some(DebugStrRef(offset)) => Some(dwarf.string(offset).ok()?.to_string().ok()?.to_string()),
        Some(unknown) => {
            println!("linkage_name_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn discr_value_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<u64>
{
    return match die.attr_value(gimli::DW_AT_discr_value).ok()? {
        Some(Data1(val)) => Some(val as u64),
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            println!("byte_size_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


pub fn artificial_attribute<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> Option<bool>
{
    return match die.attr_value(gimli::DW_AT_artificial).ok()? {
        Some(Flag(val)) => Some(val),
        Some(unknown) => {
            println!("artificial_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}


