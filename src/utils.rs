use gimli::{DebuggingInformationEntry, Dwarf, Error, Range, RangeIter, Reader, Unit};
use log::error;

/// Check if the given address is withing range of any of the given ranges.
///
/// Description:
///
/// * `pc` - A 32 bit machine code address, which is most commonly the current program counter value.
/// * `rang` - A iterator over machine code address ranges.
///
/// It checks if the given address is within the range of each given address ranges.
/// If the address is in range of one of them then it will return `Some(true)`, otherwise it will return
/// `Some(false)`.
/// The function will only return `None` if the address range iterator does not contain any address
/// ranges.
pub fn in_ranges<R>(pc: u32, rang: &mut RangeIter<R>) -> Option<bool>
where
    R: Reader<Offset = usize>,
{
    let mut no_range = true;
    while let Ok(Some(range)) = rang.next() {
        if in_range(pc, &range) {
            return Some(true);
        }
        no_range = false;
    }
    if no_range {
        return None;
    }
    return Some(false);
}

/// Check if the given address is withing a range of addresses.
///
/// Description:
///
/// * `pc` - A 32 bit machine code address, which is most commonly the current program counter value.
/// * `range` - A reference to a machine code address range.
///
/// It checks if the given address is within the range of machine code addresses.
/// If the address is in range it will return `true`, otherwise `false`.
/// return false.
pub fn in_range(pc: u32, range: &Range) -> bool {
    range.begin <= pc as u64 && range.end > pc as u64
}

/// Check if the given address is withing a DIEs address range.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs Dwarf struct.
/// * `unit` - A reference to a gimli-rs Unit struct, which contains the DIE to check.
/// * `die` - A reference to a gimli-rs Die struct that will be checked.
/// * `pc` - A 32 bit machine code address, which is most commonly the current program counter value.
///
/// It checks if the given address is within the address range of the given DIE.
/// If the address is in range it will return `Some(true)`, otherwise it will return `Some(false)`.
/// If the DIE has no address ranges it will return `None`.
pub fn die_in_range<'a, R>(
    dwarf: &'a Dwarf<R>,
    unit: &'a Unit<R>,
    die: &DebuggingInformationEntry<'_, '_, R>,
    pc: u32,
) -> Option<bool>
where
    R: Reader<Offset = usize>,
{
    match dwarf.die_ranges(unit, die) {
        Ok(mut range) => in_ranges(pc, &mut range),
        Err(_) => None,
    }
}

/// Find a compilation unit(gimli-rs Unit) using a address.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs Dwarf struct.
/// * `pc` - A 32 bit machine code address, which is most commonly the current program counter value.
///
/// This function will check if the given address is within range of all the compilation units in the `.debug_info` DWARF section.
/// If there is only one unit in range it will return it, otherwise it will return a error.
pub fn get_current_unit<'a, R>(dwarf: &'a Dwarf<R>, pc: u32) -> Result<Unit<R>, Error>
where
    R: Reader<Offset = usize>,
{
    // TODO: Maybe return a Vec of units
    let mut res = None;

    let mut iter = dwarf.units();
    let mut i = 0;
    while let Some(header) = iter.next()? {
        let unit = dwarf.unit(header)?;
        if Some(true) == in_ranges(pc, &mut dwarf.unit_ranges(&unit)?) {
            res = Some(unit);
            i += 1;
        }
    }

    if i > 1 {
        error!("Found more then one unit in range {}", i);
    }

    return match res {
        Some(u) => Ok(u),
        None => Err(Error::MissingUnitDie),
    };
}
