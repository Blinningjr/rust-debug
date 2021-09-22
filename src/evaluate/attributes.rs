use gimli::{
    AttributeValue::{
        AddressClass, Data1, Data2, Data4, Data8, DebugStrRef, Encoding, Sdata, Udata,
    },
    DebuggingInformationEntry, DwAddr, DwAte, Reader, Unit,
};

use anyhow::Result;

/// This function will return the value of the name attribute in the given DIE.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_name` from the given DIE.
pub fn name_attribute<R: Reader<Offset = usize>>(
    dwarf: &gimli::Dwarf<R>,
    die: &DebuggingInformationEntry<R>,
) -> Option<String> {
    return match die.attr_value(gimli::DW_AT_name).ok()? {
        Some(DebugStrRef(offset)) => Some(dwarf.string(offset).ok()?.to_string().ok()?.to_string()),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        None => return None,
    };
}

/// This function will return the value of the byte_size attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_byte_size` from the given DIE.
pub fn byte_size_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<u64> {
    return match die.attr_value(gimli::DW_AT_byte_size).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the alignment attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_alignment` from the given DIE.
pub fn alignment_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<u64> {
    return match die.attr_value(gimli::DW_AT_alignment).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the data_member_location attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_data_member_location` from the given DIE.
pub fn data_member_location_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<u64> {
    return match die.attr_value(gimli::DW_AT_data_member_location).ok()? {
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the type attribute in the given DIE.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A reference to gimli-rs `Unit` struct which contains the given DIE.
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_type` from the given DIE.
pub fn type_attribute<R: Reader<Offset = usize>>(
    dwarf: &gimli::Dwarf<R>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
) -> Result<Option<(gimli::UnitSectionOffset, gimli::UnitOffset)>> {
    match die.attr_value(gimli::DW_AT_type)? {
        Some(gimli::AttributeValue::UnitRef(offset)) => {
            return Ok(Some((unit.header.offset(), offset)));
        }
        Some(gimli::AttributeValue::DebugInfoRef(di_offset)) => {
            let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
            let mut iter = dwarf.debug_info.units();
            while let Ok(Some(header)) = iter.next() {
                let unit = dwarf.unit(header)?;
                if let Some(offset) = offset.to_unit_offset(&unit) {
                    return Ok(Some((unit.header.offset(), offset)));
                }
            }
            return Ok(None);
        }
        _ => return Ok(None),
    };
}

/// This function will return the value of the address_class attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_address_class` from the given DIE.
pub fn address_class_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<DwAddr> {
    return match die.attr_value(gimli::DW_AT_address_class).ok()? {
        Some(AddressClass(val)) => Some(val),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the const_value attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_cont_value` from the given DIE.
pub fn const_value_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<u64> {
    return match die.attr_value(gimli::DW_AT_const_value).ok()? {
        Some(Udata(val)) => Some(val),
        Some(Sdata(val)) => Some(val as u64), // TODO: Should not be converted to unsigned
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the count attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_count` from the given DIE.
pub fn count_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<u64> {
    return match die.attr_value(gimli::DW_AT_count).ok()? {
        Some(Udata(val)) => Some(val),
        Some(Data1(val)) => Some(val as u64),
        Some(Data2(val)) => Some(val as u64),
        Some(Data4(val)) => Some(val as u64),
        Some(Data8(val)) => Some(val),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the encoding attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_encoding` from the given DIE.
pub fn encoding_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<DwAte> {
    return match die.attr_value(gimli::DW_AT_encoding).ok()? {
        Some(Encoding(val)) => Some(val),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the discr attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_discr` from the given DIE.
pub fn discr_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<gimli::UnitOffset> {
    return match die.attr_value(gimli::DW_AT_discr).ok()? {
        Some(gimli::AttributeValue::UnitRef(offset)) => Some(offset),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}

/// This function will return the value of the discr_value attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_discr_value` from the given DIE.
pub fn discr_value_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Option<u64> {
    return match die.attr_value(gimli::DW_AT_discr_value).ok()? {
        Some(Data1(val)) => Some(val as u64),
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            panic!("Unimplemented for {:?}", unknown);
        }
        _ => None,
    };
}
