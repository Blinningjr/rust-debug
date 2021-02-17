//! Testing how to parse and evaluate dwarf information.


mod debugger;

use debugger::{
    Debugger,
    utils::{
        in_ranges
    },
};

use std::{borrow, env, fs};
use std::path::Path;

use probe_rs::{
    Probe,
    Core,
};

use probe_rs::flashing::{
    Format,
    download_file,
};

use core::time::Duration;

use object::{Object, ObjectSection};

use gimli::{
    Unit,
    Dwarf,
    Error,
    DebuggingInformationEntry,
    Reader,
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

    read_dwarf(pc_value, core, path);
    Ok(())
}


fn read_dwarf(pc: u32, core: Core, path: &Path) {
    let file = fs::File::open(&path).unwrap();
    let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };
    let object = object::File::parse(&*mmap).unwrap();
    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    dump_file(object, endian, pc, core);
}


fn dump_file(object: object::File, endian: gimli::RunTimeEndian, pc: u32, core: Core) -> Result<(), gimli::Error> {
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
    let borrow_section: &dyn for<'b> Fn(
        &'b borrow::Cow<[u8]>,
    ) -> gimli::EndianSlice<'b, gimli::RunTimeEndian> =
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

    let mut debugger = Debugger::new(core, dwarf, &unit, pc);
    
    let search = "my_num";
    let value = debugger.find_variable(search); 
    println!("var {:?} = {:#?}", search, value);

    println!("################");
    
    let search = "test_struct";
    let value = debugger.find_variable(search); 
    println!("var {:?} = {:#?}", search, value);

    println!("################");

    let search = "test_enum1";
    let value = debugger.find_variable(search); 
    println!("var {:?} = {:#?}", search, value);

    println!("################");
    
    let search = "test_enum2";
    let value = debugger.find_variable(search); 
    println!("var {:?} = {:#?}", search, value);

    println!("################");
    
    let search = "test_enum3";
    let value = debugger.find_variable(search); 
    println!("var {:?} = {:#?}", search, value);
 
    return Ok(());
}


fn get_current_unit<'a, R>(
        dwarf: &'a Dwarf<R>,
        pc: u32
    ) -> Result<Unit<R>, Error>
        where R: Reader<Offset = usize>
{
    // TODO: Maybe return a Vec of units
    let mut res = None;

    let mut iter = dwarf.units();
    let mut i = 0;
    while let Some(header) = iter.next()? {
        let unit = dwarf.unit(header)?;
        if Some(true) == in_ranges(pc, &mut dwarf.unit_ranges(&unit).unwrap()) {
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
        if Some(true) == in_ranges(pc, &mut dwarf.die_ranges(unit, entry)?) {
            dies.push(entry.clone());
        }
    }
    return Ok(dies);
}

