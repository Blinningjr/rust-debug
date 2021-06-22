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
        None => return None,
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


pub fn discr_attribute<R: Reader<Offset = usize>>(
        die: &DebuggingInformationEntry<R>
    ) -> Option<gimli::UnitOffset>
{
    return match die.attr_value(gimli::DW_AT_discr).ok()? {
        Some(gimli::AttributeValue::UnitRef(offset)) => Some(offset),
        Some(unknown) => {
            println!("discr_attribute, unknown: {:?}", unknown);
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
            println!("discr_value_attribute, unknown: {:?}", unknown);
            unimplemented!();
        },
        _ => None,
    };
}

