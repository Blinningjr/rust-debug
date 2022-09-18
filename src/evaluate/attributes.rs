use gimli::{
    AttributeValue::{
        AddressClass, Data1, Data2, Data4, Data8, DebugStrRef, Encoding, Sdata, Udata,
    },
    DebuggingInformationEntry, DwAddr, DwAte, Reader, Unit,
};

use anyhow::{anyhow, Result};
use log::error;

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
) -> Result<Option<String>> {
    match die.attr_value(gimli::DW_AT_name)? {
        Some(DebugStrRef(offset)) => Ok(Some(dwarf.string(offset)?.to_string()?.to_string())),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            Err(anyhow!("Unimplemented for {:?}", unknown))
        }
        None => Ok(None),
    }
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
) -> Result<Option<u64>> {
    match die.attr_value(gimli::DW_AT_byte_size)? {
        Some(Udata(val)) => Ok(Some(val)),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            Err(anyhow!("Unimplemented for {:?}", unknown))
        }
        _ => Ok(None),
    }
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
) -> Result<Option<u64>> {
    match die.attr_value(gimli::DW_AT_alignment)? {
        Some(Udata(val)) => Ok(Some(val)),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            Err(anyhow!("Unimplemented for {:?}", unknown))
        }
        _ => Ok(None),
    }
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
) -> Result<Option<u64>> {
    match die.attr_value(gimli::DW_AT_data_member_location)? {
        Some(Udata(val)) => Ok(Some(val)),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            Err(anyhow!("Unimplemented for {:?}", unknown))
        }
        _ => Ok(None),
    }
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
        Some(gimli::AttributeValue::UnitRef(offset)) => Ok(Some((unit.header.offset(), offset))),
        Some(gimli::AttributeValue::DebugInfoRef(di_offset)) => {
            let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
            let mut iter = dwarf.debug_info.units();
            while let Ok(Some(header)) = iter.next() {
                let unit = dwarf.unit(header)?;
                if let Some(offset) = offset.to_unit_offset(&unit) {
                    return Ok(Some((unit.header.offset(), offset)));
                }
            }
            error!("Could not find type attribute value");
            Ok(None)
        }
        _ => Ok(None),
    }
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
) -> Result<Option<DwAddr>> {
    match die.attr_value(gimli::DW_AT_address_class)? {
        Some(AddressClass(val)) => Ok(Some(val)),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            Err(anyhow!("Unimplemented for {:?}", unknown))
        }
        _ => Ok(None),
    }
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
) -> Result<Option<u64>> {
    Ok(match die.attr_value(gimli::DW_AT_const_value)? {
        Some(Udata(val)) => Some(val),
        Some(Sdata(val)) => Some(val as u64), // TODO: Should not be converted to unsigned
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            return Err(anyhow!("Unimplemented for {:?}", unknown));
        }
        _ => None,
    })
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
) -> Result<Option<u64>> {
    Ok(match die.attr_value(gimli::DW_AT_count)? {
        Some(Udata(val)) => Some(val),
        Some(Data1(val)) => Some(val as u64),
        Some(Data2(val)) => Some(val as u64),
        Some(Data4(val)) => Some(val as u64),
        Some(Data8(val)) => Some(val),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            return Err(anyhow!("Unimplemented for {:?}", unknown));
        }
        _ => None,
    })
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
) -> Result<Option<DwAte>> {
    Ok(match die.attr_value(gimli::DW_AT_encoding)? {
        Some(Encoding(val)) => Some(val),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            return Err(anyhow!("Unimplemented for {:?}", unknown));
        }
        _ => None,
    })
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
) -> Result<Option<gimli::UnitOffset>> {
    Ok(match die.attr_value(gimli::DW_AT_discr)? {
        Some(gimli::AttributeValue::UnitRef(offset)) => Some(offset),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            return Err(anyhow!("Unimplemented for {:?}", unknown));
        }
        _ => None,
    })
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
) -> Result<Option<u64>> {
    Ok(match die.attr_value(gimli::DW_AT_discr_value)? {
        Some(Data1(val)) => Some(val as u64),
        Some(Udata(val)) => Some(val),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            return Err(anyhow!("Unimplemented for {:?}", unknown));
        }
        _ => None,
    })
}

/// This function will return the value of the lower_bound attribute in the given DIE.
///
/// Description:
///
/// * `die` - A reference to a gimli-rs `Die` struct.
///
/// This function will try to retrieve the value of the attribute `DW_AT_lower_bound` from the given DIE.
pub fn lower_bound_attribute<R: Reader<Offset = usize>>(
    die: &DebuggingInformationEntry<R>,
) -> Result<Option<u64>> {
    Ok(match die.attr_value(gimli::DW_AT_lower_bound)? {
        Some(Data1(val)) => Some(val as u64),
        Some(Udata(val)) => Some(val),
        Some(Sdata(val)) => Some(val as u64),
        Some(unknown) => {
            error!("Unimplemented for {:?}", unknown);
            return Err(anyhow!("Unimplemented for {:?}", unknown));
        }
        _ => None,
    })
}
