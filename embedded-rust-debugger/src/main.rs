mod commands;
mod rust_debug;
mod debugger;
mod cli;
mod debug_adapter;


use rust_debug::{
    utils::{
        in_ranges
    },
};

use std::{borrow, fs};
use std::path::Path;

use probe_rs::{
    Probe,
    Session,
};

use object::{Object, ObjectSection};

use gimli::{
    Unit,
    Dwarf,
    Error,
    Reader,
    DebugFrame,
    LittleEndian,
    read::EndianRcSlice,
    Section,
};

use std::rc::Rc;

use std::path::PathBuf;
use structopt::StructOpt;

use anyhow::{
    Context,
    Result,
    anyhow,
};

use std::str::FromStr;


use simplelog::*;

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
pub struct Opt {
    /// Set Mode
    #[structopt(short = "m", long = "mode", default_value = "Debug")]
    mode: Mode,

    /// Set log level
    #[structopt(short = "v", long = "verbosity", default_value = "Off")]
    verbosity: LevelFilter,

    /// Dwarf file path: only required when `mode` is set to `Debug`
    #[structopt(name = "FILE", required_if("mode", "Debug"), parse(from_os_str))]
    file_path: Option<PathBuf>,

    /// Set Port: only required when `mode` is set to `DebugAdapter`
    #[structopt(short = "p", long = "port", required_if("mode", "DebugAdapter"), default_value = "8800")]
    port: u16,
}


fn main() -> Result<()> {
    let opt = Opt::from_args();
    
    // Setup log
    let cfg = ConfigBuilder::new().build();
    let log_level = opt.verbosity;
    let _ = TermLogger::init(log_level, cfg, TerminalMode::Mixed);
    
    match opt.mode {
        Mode::Debug => cli::debug_mode(opt),
        Mode::DebugAdapter => debug_adapter::start_tcp_server(opt.port),
    }
}


fn attach_probe(chip: &str, probe_num: usize) -> Result<Session>
{
    // Get a list of all available debug probes.
    let probes = Probe::list_all();

    // Use the first probe found.
    let probe = match probes.len() > probe_num {
        true => probes[probe_num].open().context("Failed to open probe")?,
        false => return Err(anyhow!("Probe {} not available", probe_num)),
    };

    // Attach to a chip.
    let session = probe.attach_under_reset(chip).context("Failed to attach probe to target")?;
 
    Ok(session)
}


fn read_dwarf<'a>(path: &Path) -> Result<(Dwarf<EndianRcSlice<LittleEndian>>, DebugFrame<EndianRcSlice<LittleEndian>>)> {
    let file = fs::File::open(&path)?;
    let mmap = unsafe { memmap::Mmap::map(&file)? };
    let object = object::File::parse(&*mmap)?;

    // Load a section and return as `Cow<[u8]>`.
    let loader = |id: gimli::SectionId| -> Result<EndianRcSlice<LittleEndian>, gimli::Error> {
        let data = object
            .section_by_name(id.name())
            .and_then(|section| section.uncompressed_data().ok())
            .unwrap_or_else(|| borrow::Cow::Borrowed(&[][..]));

        Ok(gimli::read::EndianRcSlice::new(
            Rc::from(&*data),
            gimli::LittleEndian,
        ))
    };

    // Load a supplementary section. We don't have a supplementary object file,
    // so always return an empty slice.
    let sup_loader = |_| {
        Ok(EndianRcSlice::new(
            Rc::from(&*borrow::Cow::Borrowed(&[][..])),
            LittleEndian,
        ))
    };

    // Load all of the sections.
    let dwarf = Dwarf::load(&loader, &sup_loader)?;

    let frame_section = DebugFrame::load(loader)?;

    Ok((dwarf, frame_section))
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

