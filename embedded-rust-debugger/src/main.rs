mod commands_temp;
mod debug_handler;
mod debug_adapter;
mod debugger;
mod newdebugger;
mod debugger_cli;
mod commands;
mod server;
mod request_command_handlers;

use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;
use std::{thread, time};

use rustyline::Editor;


use commands_temp::{
    debug_request::DebugRequest,
    debug_response::DebugResponse,
    Command,
};

use debugserver_types::{
    Response,
    Request, 
};

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
    DebugFrame,
    LittleEndian,
    read::EndianRcSlice,
    Section,
};

use std::rc::Rc;

use std::path::PathBuf;
use structopt::StructOpt;

use debugger_cli::DebuggerCli;

use anyhow::{Context, Result};

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
        Mode::Debug => new_debug_mode(opt), //debug_mode(opt.file_path.unwrap()),
        Mode::DebugAdapter => debug_adapter::start_tcp_server(opt.port),//server::start_server(opt.port),
    }
}


fn new_debug_mode(opt: Opt) -> Result<()> {
    let (sender_to_cli, cli_receiver): (Sender<bool>, Receiver<bool>) = mpsc::channel();
    let (sender_to_control_center, control_center_receiver): (Sender<Command>, Receiver<Command>) = mpsc::channel();
    let (sender_to_debugger, debug_receiver): (Sender<DebugRequest>, Receiver<DebugRequest>) = mpsc::channel();

    let debug_sender = sender_to_control_center.clone();

    let debugger_th = thread::spawn(move || {
        let mut debugger = debug_handler::DebugHandler::new(Some(opt));
        debugger.run(debug_sender, debug_receiver).unwrap();
    });

    let reader_th = thread::spawn(move || {
        command_reader(sender_to_control_center, cli_receiver).unwrap();
    });
    

    control_center(sender_to_debugger, control_center_receiver, sender_to_cli)?;
    debugger_th.join().expect("oops! the child thread panicked");
    reader_th.join().expect("oops! the child thread panicked");

    Ok(())
}


fn command_reader(sender: Sender<Command>,
                  receiver: Receiver<bool>
                  ) -> Result <()>
{
    let mut rl = Editor::<()>::new(); 
    let cmd_parser = commands_temp::commands::Commands::new();

    loop {
        let readline = rl.readline(">> ");

        match readline {
            Ok(line) => {
                let history_entry: &str = line.as_ref();
                rl.add_history_entry(history_entry);

                if let Some(help_string) = cmd_parser.check_if_help(history_entry) {
                    println!("{}", help_string);
                    continue;
                }
    
                let request = match cmd_parser.parse_command(line.as_ref()) {
                    Ok(cmd) => cmd,
                    Err(err) => {
                        println!("Error: {:?}", err);
                        continue;
                    },
                };

                sender.send(request)?;
                let exit = receiver.recv()?;

                if exit {
                        return Ok(());
                }
            }
            Err(e) => {
                use rustyline::error::ReadlineError;
    
                match e {
                    // For end of file and ctrl-c, we just quit
                    ReadlineError::Eof | ReadlineError::Interrupted => return Ok(()),
                    actual_error => {
                        // Show error message and quit
                        println!("Error handling input: {:?}", actual_error);
                        return Ok(());
                    }
                }
            }
        }
    }


}


fn control_center(debug_sender: Sender<DebugRequest>,
                  receiver: Receiver<Command>,
                  cli_sender: Sender<bool>
                  ) -> Result<()>
{
    loop {
        let command = receiver.recv()?;
        match command {
            Command::Request(req) => {
                debug_sender.send(req)?;
            },
            Command::Response(res) => {
                println!("{:?}", res);
                match res {
                    DebugResponse::Exit => {
                        cli_sender.send(true)?;
                        return Ok(());
                    },
                    _ => {
                        cli_sender.send(false)?;
                    },
                };
            },
            Command::Event(event) => println!("{:?}", event),
        };
    }
}


fn debug_mode(file_path: PathBuf) -> Result<()>
{
    let (cli_sender, debugger_receiver): (Sender<String>, Receiver<String>) = mpsc::channel();
    let (debugger_sender, cli_receiver): (Sender<bool>, Receiver<bool>) = mpsc::channel();
    let (debug_check_sender, status_check_receiver): (Sender<bool>, Receiver<bool>) = mpsc::channel();
    let status_check_sender = cli_sender.clone();

    let debugger_th = thread::spawn(move || {
            let mut session = attach_probe("STM32F411RETx", 0).unwrap();

            let _pc = flash_target(&mut session, &file_path).unwrap();

            let (owned_dwarf, owned_debug_frame) = read_dwarf(&file_path).unwrap();

            let debugger = Debugger::new(&owned_dwarf, &owned_debug_frame);

            let mut ndbug = newdebugger::NewDebugger::new(debugger, session).unwrap();

            ndbug.run(debugger_sender, debugger_receiver, debug_check_sender).unwrap();
        });

    let status_check_th = thread::spawn(move || {
            loop {
                let timeout = time::Duration::from_millis(200);
                let now = time::Instant::now();
                
                thread::sleep(timeout);

                status_check_sender.send("__checkhitbreakpoint__".to_string()).unwrap();

                if status_check_receiver.recv().unwrap() {
                    return ();
                } 
            }
        });

    let mut rl = Editor::<()>::new();
    
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                let history_entry: &str = line.as_ref();
                rl.add_history_entry(history_entry);
        
                cli_sender.send(line)?;
                let exit_cli = cli_receiver.recv()?;
                
                if exit_cli {
                    debugger_th.join().expect("oops! the child thread panicked");
                    return Ok(());
                }
            }
            Err(e) => {
                use rustyline::error::ReadlineError;
    
                match e {
                    // For end of file and ctrl-c, we just quit
                    ReadlineError::Eof | ReadlineError::Interrupted => return Ok(()),
                    actual_error => {
                        // Show error message and quit
                        println!("Error handling input: {:?}", actual_error);
                        return Ok(());
                    }
                }
            }
        }
    }
}


fn attach_probe(chip: &str, probe_num: usize) -> Result<Session>
{
    // Get a list of all available debug probes.
    let probes = Probe::list_all();
    
    // Use the first probe found.
    let probe = probes[probe_num].open().context("Failed to open probe")?;
    
    // Attach to a chip.
    let session = probe.attach_under_reset(chip).context("Failed to attach probe to target")?; // TODO: User should choose.
 
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

