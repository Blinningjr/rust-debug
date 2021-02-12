use gimli::{
    RangeIter,
    Unit,
    Dwarf,
    DebuggingInformationEntry,
    AttributeValue::{
        DebugStrRef,
        UnitRef,
    },
    Reader,
    EntriesTreeNode,
    Value,
    Error,
};


pub fn in_range<R>(pc: u32, rang: &mut RangeIter<R>) -> Option<bool>
        where R: Reader<Offset = usize>
{ 
    let mut no_range = true;
    while let Ok(Some(range)) = rang.next() {
//        println!("range: {:?}", range);
        if range.begin <= pc as u64 && range.end >= pc as u64 {
            return Some(true);
        }
        no_range = false;
    }
    if no_range {
        return None;
    }
    return Some(false);
}


pub fn die_in_range<'a, R>(
        dwarf: &'a Dwarf<R>,
        unit: &'a Unit<R>,
        die: &DebuggingInformationEntry<'_, '_, R>,
        pc: u32,)
    -> Option<bool>
        where R: Reader<Offset = usize>
{
    match dwarf.die_ranges(unit, die) {
        Ok(mut range) => in_range(pc, &mut range),
        Err(_) => None,
    }
}

