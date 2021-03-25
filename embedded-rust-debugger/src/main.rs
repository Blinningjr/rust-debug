mod debugger;
mod debugger_cli;
mod commands;

use debugger::{
    Debugger,
    utils::{
        in_ranges
    },
};

use std::{borrow, fs};
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


use object::{Object, ObjectSection};

use gimli::{
    Unit,
    Dwarf,
    Error,
    Reader,
};

use std::path::PathBuf;
use structopt::StructOpt;

use debugger_cli::DebuggerCli;

use anyhow::{Context, Result};

use std::str::FromStr;
use std::string::ParseError;

#[derive(Debug)]
enum Mode {
    Debug,
    DebugAdapter,
}

impl FromStr for Mode {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Debug"         => Ok(Mode::Debug),
            "debug"         => Ok(Mode::Debug),
            "DebugAdapter"  => Ok(Mode::DebugAdapter),
            "server"        => Ok(Mode::DebugAdapter),
            _               => Err("Error: invalid mode"),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opt {
    /// Set Mode
    #[structopt(short = "m", long = "mode", default_value = "Debug")]
    mode: Mode,

    /// Dwarf file path: only required when `mode` is set to `Debug`
    #[structopt(name = "FILE", required_if("mode", "Debug"), parse(from_os_str))]
    file_path: Option<PathBuf>,

    /// Set Port: only required when `mode` is set to `DebugAdapter`
    #[structopt(short = "p", long = "port", required_if("mode", "DebugAdapter"), default_value = "8800")]
    port: u16,

}


fn main() -> Result<()> {
    let opt = Opt::from_args();
    match opt.mode {
        Mode::Debug => debug_mode(opt.file_path.unwrap()),
        Mode::DebugAdapter => Ok(()),
    }
}


fn debug_mode(file_path: PathBuf) -> Result<()>
{
    let mut session = attach_probe()?;

    let _pc = flash_target(&mut session, &file_path)?;

    let core = session.core(0).unwrap();
    read_dwarf(core, &file_path)
}


fn attach_probe() -> Result<Session>
{
    // Get a list of all available debug probes.
    let probes = Probe::list_all();
    
    // Use the first probe found.
    let probe = probes[0].open().context("Failed to open probe")?; // TODO: User should choose.
    
    // Attach to a chip.
    let session = probe.attach_under_reset("STM32F411RETx").context("Failed to attach probe to target")?; // TODO: User should choose.
 
    Ok(session)
}


fn flash_target(session: &mut Session,
                file_path: &PathBuf
                ) -> Result<u32>
{
    download_file(session, file_path, Format::Elf).context("Failed to flash target")?;

    let mut core = session.core(0)?;
    let pc = core.reset_and_halt(std::time::Duration::from_millis(10)).context("Failed to reset and halt the core")?.pc;
 
    Ok(pc)
}


fn read_dwarf(core: Core, path: &Path) -> Result<()> {
    let file = fs::File::open(&path)?;
    let mmap = unsafe { memmap::Mmap::map(&file)? };
    let object = object::File::parse(&*mmap)?;
    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    dwarf_cli(object, endian, core)?;

    Ok(())
}


fn dwarf_cli(object: object::File, endian: gimli::RunTimeEndian, core: Core) -> Result<()> {
    // Load a section and return as `Cow<[u8]>`.
    let loader = |id: gimli::SectionId| -> Result<borrow::Cow<[u8]>> {
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

    let debugger = Debugger::new(core, dwarf);

    let mut cli = DebuggerCli::new(debugger)?;
    cli.run()?;
 
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

