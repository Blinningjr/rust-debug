use gimli::{
    RangeIter,
    Range,
    Unit,
    Dwarf,
    DebuggingInformationEntry,
    Reader,
};

pub fn in_ranges<R>(pc:     u32,
                    rang:   &mut RangeIter<R>
                    ) -> Option<bool>
                    where R: Reader<Offset = usize>
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


pub fn in_range(pc:     u32,
                range:  &Range
                ) -> bool
{ 
    range.begin <= pc as u64 && range.end >= pc as u64 
}


pub fn die_in_range<'a, R>(dwarf:   &'a Dwarf<R>,
                           unit:    &'a Unit<R>,
                           die:     &DebuggingInformationEntry<'_, '_, R>,
                           pc:      u32
                           )-> Option<bool>
                           where R: Reader<Offset = usize>
{
    match dwarf.die_ranges(unit, die) {
        Ok(mut range) => in_ranges(pc, &mut range),
        Err(_) => None,
    }
}

