use gimli::{DebuggingInformationEntry, Dwarf, Error, Range, RangeIter, Reader, Unit};

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

pub fn in_range(pc: u32, range: &Range) -> bool {
    range.begin <= pc as u64 && range.end > pc as u64
}

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
        panic!("Found more then one unit in range {}", i);
    }

    return match res {
        Some(u) => Ok(u),
        None => Err(Error::MissingUnitDie),
    };
}
