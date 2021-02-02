//! Testing how to parse and evaluate dwarf information.


use std::{borrow, env, fs};
use std::path::Path;

use probe_rs::{
    Probe,
    Core,
    MemoryInterface,
};
use probe_rs::flashing::{
    Format,
    download_file,
};
use core::time::Duration;

use object::{Object, ObjectSection};

use gimli::{
    EndianSlice,
    RunTimeEndian,
    RangeIter,
    Unit,
    Dwarf,
    Error,
    DebuggingInformationEntry,
    EvaluationResult::{
        Complete,
        RequiresMemory,
        RequiresRegister,
        RequiresFrameBase,
        RequiresTls,
        RequiresCallFrameCfa,
        RequiresAtLocation,
        RequiresEntryValue,
        RequiresParameterRef,
        RequiresRelocatedAddress,
        RequiresIndexedAddress,
        RequiresBaseType,
    },
    AttributeValue::{
        DebugStrRef,
        UnitRef,
        DebugLineRef,
        RangeListsRef,
        Udata,
        Encoding,
    },
    Attribute,
    ReaderOffset,
    Reader,
    EntriesTreeNode,
    Evaluation,
    EvaluationResult,
    UnitOffset,
    Register,
    Value,
    DwAte,
    Expression,
    Piece,
    Location,
};


fn main() {
    probe_rs_stuff().unwrap();
}


fn probe_rs_stuff() -> Result<(), &'static str> {
    // Get a list of all available debug probes.
    let probes = Probe::list_all();
    
    // Use the first probe found.
    let probe = probes[0].open().map_err(|_| "Failed to open probe")?;
    
    // Attach to a chip.
    let mut session = probe.attach_under_reset("STM32F411RETx").map_err(|_| "Failed to attach probe to target")?;


//    println!("{:#?}", core.registers().PC());
//    println!("{:#?}", core.registers().program_counter().address);
    let path_str = env::args().skip(1).next().unwrap();
    let path = Path::new(&path_str);
    println!("{:#?}", path);
    
    download_file(&mut session, &path, Format::Elf).map_err(|_| "Failed to flash target");
    
    let mut core = session.core(0).unwrap();

    core.reset().map_err(|_| "Faild to reset")?;

    core.wait_for_core_halted(Duration::new(5, 0)).map_err(|_| "Core never halted");

    let pc_value: u32 = core
        .read_core_reg(core.registers().program_counter())
        .unwrap();

    println!("{:#02x}", pc_value);

    read_dwarf(pc_value, &mut core, path);
    Ok(())
}


fn read_dwarf(pc: u32, core: &mut Core, path: &Path) {
    let file = fs::File::open(&path).unwrap();
    let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };
    let object = object::File::parse(&*mmap).unwrap();
    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    dump_file(&object, endian, pc, core).unwrap();
}


fn dump_file(object: &object::File, endian: gimli::RunTimeEndian, pc: u32, core: &mut Core) -> Result<(), gimli::Error> {
    // Load a section and return as `Cow<[u8]>`.
    let loader = |id: gimli::SectionId| -> Result<borrow::Cow<[u8]>, gimli::Error> {
        match object.section_by_name(id.name()) {
            Some(ref section) => Ok(section
                .uncompressed_data()
                .unwrap_or(borrow::Cow::Borrowed(&[][..]))),
            None => Ok(borrow::Cow::Borrowed(&[][..])),
        }
    };

    // Load a supplementary section. We don't have a supplementary object file,
    // so always return an empty slice.
    let sup_loader = |_| Ok(borrow::Cow::Borrowed(&[][..]));

    // Load all of the sections.
    let dwarf_cow = gimli::Dwarf::load(&loader, &sup_loader)?;

    // Borrow a `Cow<[u8]>` to create an `EndianSlice`.
    let borrow_section: &dyn for<'a> Fn(
        &'a borrow::Cow<[u8]>,
    ) -> gimli::EndianSlice<'a, gimli::RunTimeEndian> =
        &|section| gimli::EndianSlice::new(&*section, endian);

    // Create `EndianSlice`s for all of the sections.
    let dwarf = dwarf_cow.borrow(&borrow_section);


    let unit = get_current_unit(&dwarf, pc)?;
    println!("{:?}", unit.name.unwrap().to_string());


//    let dies = get_current_dies(&dwarf, &unit, pc)?;
//    println!("{:#?}", dies.iter().map(|d| d.tag().static_string()).collect::<Vec<_>>());
//
//    for die in dies.iter() {
//        check_die(&dwarf, &unit, die, pc);
//    }


    let mut tree = unit.entries_tree(None)?;
    let root = tree.root()?;
    process_tree(core, root, &dwarf, &unit, pc, false, None)?;
    
    fn process_tree<R>(
            core: &mut Core,
            mut node: EntriesTreeNode<R>,
            dwarf: &Dwarf<R>,
            unit: &'_ Unit<R>,
            pc: u32,
            prev_in_range: bool,
            mut frame_base: Option<u64>
        ) -> gimli::Result<()>
            where R: Reader<Offset = usize>
    {
        let die = node.entry();
        let in_range = die_in_range(dwarf, unit, die, pc);
        let mut in_r = true;
        match (in_range, prev_in_range) {
            (Some(false), _ ) => in_r = false, //return Ok(()),
            (None, false) => in_r = false, //return Ok(()),
            _ => (),
        };
        println!("in_r: {:?}", in_r);
        if let Some(fb) = check_die(core, dwarf, unit, die, pc, frame_base) {
            frame_base = Some(fb);
        }
        if in_r {
            let mut children = node.children();
            while let Some(child) = children.next()? {
                // Recursively process a child.
                process_tree(core, child, dwarf, unit, pc, in_r, frame_base)?;
            }
        }
        Ok(())
    }

    return Ok(());
}


fn check_die<R>(
        core: &mut Core,
        dwarf: &Dwarf<R>,
        unit: &Unit<R>,
        die: &DebuggingInformationEntry<'_, '_, R>, //&DebuggingInformationEntry<'_, '_, EndianSlice<'_, RunTimeEndian>, usize>,
        pc: u32,
        mut frame_base: Option<u64>
    ) -> Option<u64>
        where R: Reader<Offset = usize>
{
    let mut found_dies: Vec<DebuggingInformationEntry<'_, '_, R>> = vec!();

    let mut attrs = die.attrs();
    println!("{:?}", die.tag().static_string());
    println!(
        "{:<30} | {:<}",
        "Name", "Value"
    );
    println!("----------------------------------------------------------------");
    while let Some(attr) = attrs.next().unwrap() {
        let val = match attr.value() {
            DebugStrRef(offset) => format!("{:?}", dwarf.string(offset).unwrap().to_string().unwrap()),
            _ => format!("{:?}", attr.value()),
        };

        println!(
            "{: <30} | {:<?}",
            attr.name().static_string().unwrap(),
            val
        );
        if let Some(expr) = attr.value().exprloc_value() {
            if attr.name() == gimli::DW_AT_frame_base {
                frame_base = match new_evaluate(core, dwarf, unit, expr, pc, frame_base).unwrap() {
                    Value::U64(v) => Some(v),
                    Value::U32(v) => Some(v as u64),
                    _ => frame_base,
                };
            } else {
                new_evaluate(core, dwarf, unit, expr, pc, frame_base);
            }
        }
        //if let UnitRef(offset) = attr.value() {
        //    found_dies.push(unit.entry(offset).unwrap());
        //    //println!("{:?}", unit.entry(offset).unwrap().tag().static_string());
        //}
    }
    println!("\n");

//    for die in found_dies.iter() {
//        check_die(dwarf, unit, die, pc);
//    }
    return frame_base;
}


fn evaluate<'a, R>(
        dwarf: &'a Dwarf<R>,
        unit: &'a Unit<R>,
        attr: Attribute<R>,
        pc: u32
    )
        where R: Reader<Offset = usize>
{
    if let Some(expr) = attr.value().exprloc_value() {
        let mut eval = expr.evaluation(unit.encoding());
        let mut result = eval.evaluate().unwrap();

        println!("{:#?}", result);
        match result {
            gimli::EvaluationResult::Complete => {
                let result = eval.result();
                println!("{:#?}", result);
            },
            _ => (),
        };
    }
}


fn get_current_unit<'a, R>(
        dwarf: &'a Dwarf<R>,
        pc: u32
    ) -> Result<Unit<R>, Error>
        where R: Reader<Offset = usize>
{
    // TODO: Maby return a vec of units

    let mut iter = dwarf.units();
    while let Some(header) = iter.next()? {
        let unit = dwarf.unit(header)?;
        if Some(true) == in_range(pc, &mut dwarf.unit_ranges(&unit).unwrap()) {
            return Ok(unit);
        }
    }
    return Err(Error::MissingUnitDie);
}


fn get_current_dies<'a, R>(
        dwarf: &'a Dwarf<R>,
        unit: &'a Unit<R>,
        pc: u32
    ) -> Result<Vec<DebuggingInformationEntry<'a, 'a, R>>, Error>
        where R: Reader<Offset = usize>
{
    let mut entries = unit.entries();
    let mut dies: Vec<DebuggingInformationEntry<R>> = vec!();
    while let Some((_, entry)) = entries.next_dfs()? {
//        println!("{:#?}", entry.tag().static_string());
        if Some(true) == in_range(pc, &mut dwarf.die_ranges(unit, entry)?) {
            dies.push(entry.clone());
        }
    }
    return Ok(dies);
}


fn in_range<R>(pc: u32, rang: &mut RangeIter<R>) -> Option<bool>
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


fn die_in_range<'a, R>(
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


fn new_evaluate<R>(
        core: &mut Core,
        dwarf: &Dwarf<R>,
        unit: &Unit<R>,
        expr: Expression<R>,
        pc: u32,
        frame_base: Option<u64>
    ) -> Result<Value, &'static str>
        where R: Reader<Offset = usize>
{
    let mut eval = expr.evaluation(unit.encoding());
    let mut result = eval.evaluate().unwrap();

    println!("{:#?}", result);
    println!("fb: {:?}", frame_base);
    loop {
        match result {
            Complete => break,
            RequiresMemory{address, size, space, base_type} =>
                resolve_requires_mem(core, unit, &mut eval, &mut result, address, size, space, base_type),
            RequiresRegister{register, base_type} => resolve_requires_reg(core, unit, &mut eval, &mut result, register, base_type),
            RequiresFrameBase => // TODO
                result = eval.resume_with_frame_base(frame_base.unwrap()).unwrap(),
            RequiresTls(_tls) => unimplemented!(), // TODO
            RequiresCallFrameCfa => unimplemented!(), // TODO
            RequiresAtLocation(_dir_ref) => unimplemented!(), // TODO
            RequiresEntryValue(e) =>
              result = eval.resume_with_entry_value(new_evaluate(core, dwarf, unit, e, pc, frame_base)?).unwrap(),
            RequiresParameterRef(_unit_offset) => unimplemented!(), // TODO
            RequiresRelocatedAddress(num) =>
                result = eval.resume_with_relocated_address(num).unwrap(), // TODO
            RequiresIndexedAddress{index, relocate} => unimplemented!(), // TODO
            RequiresBaseType(unit_offset) => 
                result = eval.resume_with_base_type(
                    parse_base_type(unit, 0, unit_offset).value_type()).unwrap(),
        };
    }

    let value = eval_pieces(core, eval.result());
    println!("Value: {:?}", value);
    value
}

fn eval_pieces<R>(
        core: &mut Core,
        pieces: Vec<Piece<R>>
    ) -> Result<Value, &'static str>
        where R: Reader<Offset = usize>
{
    // TODO: What should happen if more then one piece is given?
    if pieces.len() > 1 {
        panic!("Found more then one piece");
    }
    println!("{:?}", pieces);
    return eval_piece(core, &pieces[0]);
}

fn eval_piece<R>(
        core: &mut Core,
        piece: &Piece<R>
    ) -> Result<Value, &'static str>
    where R: Reader<Offset = usize>
{
    // TODO: Handle size_in_bits and bit_offset
    match &piece.location {
        Location::Empty => return Err("Optimized out"),
        Location::Register { register } => // TODO Always return U32?
            return Ok(Value::U32(core.read_core_reg(register.0).unwrap())),
        Location::Address { address } =>  // TODO Always return U32?
            return Ok(Value::U32(core.read_word_32(*address as u32).map_err(|e| "Read error")?)),
        Location::Value { value } => return Ok(value.clone()),
        Location::Bytes { value } => unimplemented!(), // TODO
        Location::ImplicitPointer { value, byte_offset } => unimplemented!(), // TODO
    };
}

fn parse_base_type<R>(
        unit: &Unit<R>,
        data: u32,
        base_type: UnitOffset<usize>
    ) -> Value
        where R: Reader<Offset = usize>
{
    if base_type.0 == 0 {
        return Value::Generic(data as u64);
    }
    let die = unit.entry(base_type).unwrap();

    // I think that the die returnd must be a base type tag.
    if die.tag() != gimli::DW_TAG_base_type {
        println!("{:?}", die.tag().static_string());
        panic!("die tag not base type");
    }

    let encoding = match die.attr_value(gimli::DW_AT_encoding) {
        Ok(Some(Encoding(dwate))) => dwate,
        _ => panic!("expected Encoding"),
    };
    let byte_size = match die.attr_value(gimli::DW_AT_byte_size) {
        Ok(Some(Udata(v))) => v,
        _ => panic!("expected Udata"),
    };
    
    // Check dwarf doc for the codes.
    match (encoding, byte_size) {
        (DwAte(7), 1) => Value::U8(data as u8),     // (unsigned, 8)
        (DwAte(7), 2) => Value::U16(data as u16),   // (unsigned, 16)
        (DwAte(7), 4) => Value::U32(data as u32),   // (unsigned, 32)
        (DwAte(7), 8) => Value::U64(data as u64),   // (unsigned, 64) TODO: Fix
        
        (DwAte(5), 1) => Value::I8(data as i8),     // (signed, 8)
        (DwAte(5), 2) => Value::I16(data as i16),   // (signed, 16)
        (DwAte(5), 4) => Value::I32(data as i32),   // (signed, 32)
        (DwAte(5), 8) => Value::I64(data as i64),   // (signed, 64) TODO: Fix
        _ => unimplemented!(),
    }
}

/*
 * Resolves requires memory when evaluating a die.
 * TODO: Test.
 */
fn resolve_requires_mem<R>(
        core: &mut Core,
        unit: &Unit<R>,
        eval: &mut Evaluation<R>,
        result: &mut EvaluationResult<R>,
        address: u64,
        size: u8,
        _space: Option<u64>,
        base_type: UnitOffset<usize>
    )
        where R: Reader<Offset = usize>
{
    let data = core.read_word_32(address as u32).unwrap();
    let value = parse_base_type(unit, data, base_type);
    *result = eval.resume_with_memory(value).unwrap();    
}

/*
 * Resolves requires register when evaluating a die.
 * TODO: Test
 */
fn resolve_requires_reg<R>(
        core: &mut Core,
        unit: &Unit<R>,
        eval: &mut Evaluation<R>,
        result: &mut EvaluationResult<R>,
        reg: Register,
        base_type: UnitOffset<usize>) 
        where R: Reader<Offset = usize>
{
    let data = core.read_core_reg(reg.0).unwrap();
    let value = parse_base_type(unit, data, base_type);
    *result = eval.resume_with_register(value).unwrap();    
}

