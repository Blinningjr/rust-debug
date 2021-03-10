mod debugger;
mod debugger_cli;
mod commands;

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
    Session,
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

use std::path::PathBuf;
use structopt::StructOpt;

use debugger_cli::DebuggerCli;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    /// Input dwarf file
    #[structopt(parse(from_os_str))]
    file: PathBuf,
}


fn main() {
    let opt = Opt::from_args();

    let mut session = match attach_probe() {
        Ok(session) => session,
        Err(err)    => panic!("Error: {:?}", err),
    };

    let pc = match flash_target(&mut session, &opt.file) {
        Ok(pc)      => pc,
        Err(err)    => panic!("Error: {:?}", err),
    };

    let mut core = session.core(0).unwrap();
    read_dwarf(pc, core, &opt.file);
}


fn attach_probe() -> Result<Session, &'static str>
{
    // Get a list of all available debug probes.
    let probes = Probe::list_all();
    
    // Use the first probe found.
    let probe = probes[0].open().map_err(|_| "Failed to open probe")?; // TODO: User should choose.
    
    // Attach to a chip.
    let session = probe.attach_under_reset("STM32F411RETx").map_err(|_| "Failed to attach probe to target")?; // TODO: User should choose.
 
    Ok(session)
}


fn flash_target(session: &mut Session,
                file_path: &PathBuf
                ) -> Result<u32, &'static str>
{
    download_file(session, file_path, Format::Elf).map_err(|_| "Failed to flash target")?;

    let mut core = session.core(0).unwrap();
    let pc = core.reset_and_halt(std::time::Duration::from_millis(10)).map_err(|_| "Failed to reset and halt the core")?.pc;
 
    Ok(pc)
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
    let _ = dwarf_cli(object, endian, pc, core);
}


fn dwarf_cli(object: object::File, endian: gimli::RunTimeEndian, pc: u32, core: Core) -> Result<(), gimli::Error> {
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

    let mut cli = match DebuggerCli::new(core, dwarf) {
        Ok(val) => val,
        Err(err)    => panic!("Error: {:?}", err),
    };
    match cli.run() {
        Ok(())    => (),
        Err(err)    => panic!("Error: {:?}", err),
    }; 
 
    return Ok(());
}


pub fn get_current_unit<'a, R>(
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

