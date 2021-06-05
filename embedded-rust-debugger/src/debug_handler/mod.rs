mod config;


use config::Config;

use super::commands_temp::{
    debug_request::DebugRequest,
    debug_response::DebugResponse,
    Command,
};

use super::Opt;

use anyhow::{
    Result,
};

use std::sync::mpsc::{Sender, Receiver};

use capstone::arch::BuildsCapstone;

use super::debugger::Debugger;

use super::{
    read_dwarf,
    attach_probe,
};

use std::path::PathBuf;

use gimli::Reader;


use probe_rs::{
    MemoryInterface,
    CoreInformation,
};

use log::{
    info,
    debug,
    warn,
};


pub struct DebugHandler {
    config: Config,
}

impl DebugHandler {
    pub fn new(opt: Opt) -> DebugHandler {
        DebugHandler {
            config: Config::new(opt),
        }
    }


    pub fn run(&mut self,
               mut sender: Sender<DebugResponse>,
               mut reciver: Receiver<DebugRequest>
              ) -> Result<()>
    {
        loop {
            let request = reciver.recv()?;
            let (exit, response) = self.handle_request(&mut sender, &mut reciver, request)?;
            sender.send(response)?;

            if exit {
                return Ok(());
            } 
        }
    }


    fn handle_request(&mut self,
                      sender: &mut Sender<DebugResponse>,
                      reciver: &mut Receiver<DebugRequest>,
                      request: DebugRequest
                      ) -> Result<(bool, DebugResponse)>
    {
        match request {
            DebugRequest::Exit => Ok((true, DebugResponse::Exit)),
            DebugRequest::SetBinary { path } => {
                self.config.binary = Some(path);
                Ok((false, DebugResponse::SetBinary))
            },
            _ => {
                let new_request = init(sender,
                                       reciver,
                                       self.config.binary.clone().unwrap(),
                                       self.config.probe_num,
                                       self.config.chip.clone().unwrap(),
                                       request)?;
                self.handle_request(sender, reciver, new_request) },
        }
    }
}

pub fn init(sender: &mut Sender<DebugResponse>,
            reciver: &mut Receiver<DebugRequest>,
            file_path: PathBuf,
            probe_number: usize,
            chip:   String,
            request:    DebugRequest
            ) -> Result<DebugRequest> {
    let cs = capstone::Capstone::new() // TODO: Set the capstone base on the arch of the chip.
        .arm()
        .mode(capstone::arch::arm::ArchMode::Thumb)
        .build()
        .expect("Failed to create Capstone object");
 
    let (owned_dwarf, owned_debug_frame) = read_dwarf(&file_path).unwrap();
    let debugger = Debugger::new(&owned_dwarf, &owned_debug_frame);

    let session = attach_probe(&chip, probe_number).unwrap();
    
    let mut debug = DebugServer {
        capstone: cs,
        debugger: debugger,
        session: session,
        breakpoints: vec!(),
    };

    debug.run(sender, reciver, request)
}



struct DebugServer<'a, R: Reader<Offset = usize>> {
    debugger:   Debugger<'a, R>,
    session:    probe_rs::Session,
    capstone:   capstone::Capstone,
    breakpoints: Vec<u32>,
}

impl<'a, R: Reader<Offset = usize>> DebugServer<'a, R> {

    pub fn run(&mut self,
               sender: &mut Sender<DebugResponse>,
               reciver: &mut Receiver<DebugRequest>,
               mut request: DebugRequest
               ) -> Result<DebugRequest> {
        loop {
            match self.handle_request(request)? {
                Command::Request(req) => return Ok(req),
                Command::Response(res) => sender.send(res)?,
            };
            request = reciver.recv()?;
        }
    }


    fn handle_request(&mut self,
                      request: DebugRequest
                      ) -> Result<Command>
    {
        match request {
            DebugRequest::Continue => self.continue_command(),
            _ => Ok(Command::Request(request)),
        }
    }


    pub fn continue_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;
   
        if status.is_halted() {
            let _cpu_info = continue_fix(&mut core, &self.breakpoints)?;
            core.run()?;    
        }
    
        info!("Core status: {:?}", core.status()?);
    
        Ok(Command::Response(DebugResponse::Continue))
    }
}


fn continue_fix(core: &mut probe_rs::Core, breakpoints: &Vec<u32>) -> Result<CoreInformation, probe_rs::Error>
{
    match core.status()? {
        probe_rs::CoreStatus::Halted(r)  => {
            match r {
                probe_rs::HaltReason::Breakpoint => {
                    let pc = core.registers().program_counter();
                    let pc_val = core.read_core_reg(pc)?;

                    let mut code = [0u8; 2];
                    core.read_8(pc_val, &mut code)?;
                    if code[1] == 190 && code[0] == 0 { // bkpt == 0xbe00 for coretex-m // TODO: is the code[0] == 0 needed?
                        // NOTE: Increment with 2 because bkpt is 2 byte instruction.
                        let step_pc = pc_val + 0x2; // TODO: Fix for other CPU types.        
                        core.write_core_reg(pc.into(), step_pc)?;

                        return core.step();
                    } else {
                        for bkpt in breakpoints {
                            if pc_val == *bkpt {
                                core.clear_hw_breakpoint(pc_val)?;

                                let res = core.step();

                                core.set_hw_breakpoint(pc_val)?;

                                return res;
                            }
                        }
                        return core.step();
                    }
                },
                _ => (),
            };
        },
        _ => (),
    };

    core.step()
}

