mod config;

use config::Config;

use super::commands::{
    debug_request::DebugRequest,
    debug_response::DebugResponse,
    debug_event::DebugEvent,
    Command,

};

use super::Opt;

use anyhow::{
    Result,
    Context,
    anyhow,
};

use crossbeam_channel::{ 
    Sender,
    Receiver,
    TryRecvError,
};

use capstone::arch::BuildsCapstone;


use super::debugger;
use super::debugger::{
    Debugger,
    find_breakpoint_location,
    get_current_stacktrace,
};

use super::{
    read_dwarf,
    attach_probe,
    get_current_unit,
};

use std::path::PathBuf;

use gimli::Reader;


use probe_rs::{
    MemoryInterface,
    CoreStatus,
};

use log::{
    info,
    warn,
};

use std::time::{Instant, Duration};

use probe_rs::flashing::{
    Format,
    download_file,
};


use debugserver_types::{
    Breakpoint,
    SourceBreakpoint,
};

use std::collections::HashMap;



pub struct DebugHandler {
    config: Config,
}

impl DebugHandler {
    pub fn new(opt: Option<Opt>) -> DebugHandler {
        DebugHandler {
            config: Config::new(opt),
        }
    }


    pub fn run(&mut self,
               mut sender: Sender<Command>,
               mut reciver: Receiver<DebugRequest>
              ) -> Result<()>
    {
        loop {
            let request = reciver.recv()?;
            let (exit, response) = match self.handle_request(&mut sender, &mut reciver, request) {
                Ok(val) => val,
                Err(err) => {
                    sender.send(Command::Response(DebugResponse::Error { message: format!("{:?}", err)}))?;
                    continue;
                },
            };
            sender.send(Command::Response(response))?;

            if exit {
                return Ok(());
            } 
        }
    }


    fn handle_request(&mut self,
                      sender: &mut Sender<Command>,
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
            DebugRequest::SetProbeNumber { number } => {
                self.config.probe_num = number;
                Ok((false, DebugResponse::SetProbeNumber))
            },
            DebugRequest::SetChip { chip } => {
                self.config.chip = Some(chip);
                Ok((false, DebugResponse::SetChip))
            },
            DebugRequest::SetCWD { cwd } => {
                self.config.cwd = Some(cwd);
                Ok((false, DebugResponse::SetCWD))
            },
            _ => {
                if self.config.is_missing_config() {
                    return Ok((false, DebugResponse::Error {
                        message: self.config.missing_config_message(),
                    }));
                }

                let new_request = init(sender,
                                       reciver,
                                       self.config.binary.clone().unwrap(),
                                       self.config.probe_num,
                                       self.config.chip.clone().unwrap(),
                                       self.config.cwd.clone().unwrap(),
                                       request)?;
                self.handle_request(sender, reciver, new_request)
            },
        }
    }
}

pub fn init(sender: &mut Sender<Command>,
            reciver: &mut Receiver<DebugRequest>,
            file_path: PathBuf,
            probe_number: usize,
            chip:   String,
            cwd:    String,
            request:    DebugRequest
            ) -> Result<DebugRequest> {
    let cs = capstone::Capstone::new() // TODO: Set the capstone base on the arch of the chip.
        .arm()
        .mode(capstone::arch::arm::ArchMode::Thumb)
        .build()
        .expect("Failed to create Capstone object");
 
    let (owned_dwarf, owned_debug_frame) = read_dwarf(&file_path)?;
    let debugger = Debugger::new(&owned_dwarf, &owned_debug_frame);

    let session = attach_probe(&chip, probe_number)?;
    
    let mut debug = DebugServer {
        capstone: cs,
        debugger: debugger,
        session: session,
        breakpoints: HashMap::new(),
        file_path: file_path,
        cwd:        cwd,
        check_time: Instant::now(),
        running: true,
    };

    debug.run(sender, reciver, request)
}



struct DebugServer<'a, R: Reader<Offset = usize>> {
    debugger:   Debugger<'a, R>,
    session:    probe_rs::Session,
    capstone:   capstone::Capstone,
    breakpoints: HashMap<u32, Breakpoint>,
    file_path:  PathBuf,
    cwd:        String,
    check_time: Instant, 
    running: bool,
}

impl<'a, R: Reader<Offset = usize>> DebugServer<'a, R> {

    pub fn run(&mut self,
               sender: &mut Sender<Command>,
               reciver: &mut Receiver<DebugRequest>,
               request: DebugRequest
               ) -> Result<DebugRequest>
    {
        match self.handle_request(request)? {
            Command::Request(req) => return Ok(req),
            Command::Response(res) => sender.send(Command::Response(res))?,
            _ => unimplemented!(),
        };

        loop {
            match reciver.try_recv() {
                Ok(request) => {
                    match self.handle_request(request)? {
                        Command::Request(req) => {
                            
                            let mut core = self.session.core(0)?;
                            core.clear_all_hw_breakpoints()?;
                            self.breakpoints = HashMap::new();

                            return Ok(req);
                        },
                        Command::Response(res) => sender.send(Command::Response(res))?,
                        _ => unimplemented!(),
                    };
                },
                Err(err) => {
                    match err {
                        TryRecvError::Empty => self.check_halted(sender)?,
                        TryRecvError::Disconnected => {

                            let mut core = self.session.core(0)?;
                            core.clear_all_hw_breakpoints()?;
                            self.breakpoints = HashMap::new();

                            return Err(anyhow!("{:?}", err));
                        },
                    };
                },
            };
        }
    }


    fn check_halted(&mut self, sender: &mut Sender<Command>) -> Result<()> {
        let delta = Duration::from_millis(400);
        if self.running && self.check_time.elapsed() > delta {
            self.check_time = Instant::now();
            self.send_halt_event(sender)?;
        }

        Ok(())
    }

    fn send_halt_event(&mut self, sender: &mut Sender<Command>) -> Result<()> {
        let mut core = self.session.core(0)?;
        let status = core.status()?;

        if let CoreStatus::Halted(reason) = status {
            self.running = false;

            let pc = core.read_core_reg(core.registers().program_counter())?;

            let mut hit_breakpoint_ids = vec!();
            match self.breakpoints.get(&pc) {
                Some(bkpt) => hit_breakpoint_ids.push(bkpt.id.unwrap() as u32),
                None => (),
            };
        
            sender.send(Command::Event(DebugEvent::Halted{
                pc: pc,
                reason: reason,
                hit_breakpoint_ids: Some(hit_breakpoint_ids)
            }))?; 
        }

        Ok(())
    }


    fn handle_request(&mut self,
                      request: DebugRequest
                      ) -> Result<Command>
    {
        match request {
            DebugRequest::Attach { reset, reset_and_halt } => self.attach_command(reset, reset_and_halt),
            DebugRequest::Stack => self.stack_command(),
            DebugRequest::Code => self.code_command(),
            DebugRequest::ClearAllBreakpoints => self.clear_all_breakpoints_command(),
            DebugRequest::ClearBreakpoint { address } => self.clear_breakpoint_command(address),
            DebugRequest::SetBreakpoint { address, source_file } => self.set_breakpoint_command(address, source_file),
            DebugRequest::Registers => self.registers_command(),
            DebugRequest::Variable { name } => self.variable_command(&name),
            DebugRequest::StackTrace      => self.stack_trace_command(),
            DebugRequest::Read { address, byte_size }     => self.read_command(address, byte_size),
            DebugRequest::Reset { reset_and_halt: rah }     => self.reset_command(rah),
            DebugRequest::Flash { reset_and_halt: rah }     => self.flash_command(rah),
            DebugRequest::Halt      => self.halt_command(),
            DebugRequest::Status    => self.status_command(),
            DebugRequest::Continue  => self.continue_command(),
            DebugRequest::Step      => self.step_command(),
            DebugRequest::SetBreakpoints { source_file, source_breakpoints } => self.set_breakpoints_command(source_file, source_breakpoints),

            _ => Ok(Command::Request(request)),
        }
    }

    fn attach_command(&mut self, reset: bool, reset_and_halt: bool) -> Result<Command>
    {
        if reset_and_halt {
            let mut core = self.session.core(0)?;
            core.reset_and_halt(std::time::Duration::from_millis(10)).context("Failed to reset and halt the core")?; 
        } else if reset {
            let mut core = self.session.core(0)?;
            core.reset().context("Failed to reset the core")?;
        }

        Ok(Command::Response(DebugResponse::Attach))
    }

    fn stack_command(&mut self) -> Result<Command>
    { 
        let mut core = self.session.core(0)?;
        let status = core.status()?;

        if status.is_halted() {
            let sp_reg: u16 = probe_rs::CoreRegisterAddress::from(core.registers().stack_pointer()).0;
 
            let sf = core.read_core_reg(7)?; // reg 7 seams to be the base stack address.
            let sp = core.read_core_reg(sp_reg)?;

            if sf < sp {
                // The previous stack pointer is less then current.
                // This happens when there is no stack.
                return Ok(Command::Response(DebugResponse::Stack {
                    stack_pointer: sp,
                    stack: vec!(),
                }));
            }

            let length = (((sf - sp) + 4 - 1)/4) as usize;
            let mut stack = vec![0u32; length];
            core.read_32(sp, &mut stack)?;
        
            return Ok(Command::Response(DebugResponse::Stack {
                stack_pointer: sp,
                stack: stack,
            }));
        } else {
            return Err(anyhow!("Core must be halted"));
        }
 
    }


    fn code_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;

        if status.is_halted() {
            let pc = core.registers().program_counter();
            let pc_val = core.read_core_reg(pc)?;

            let mut code = [0u8; 16 * 2];

            core.read_8(pc_val, &mut code)?;

            let insns = self.capstone.disasm_all(&code, pc_val as u64)
                .expect("Failed to disassemble");

            let mut instructions = vec!();
            for i in insns.iter() {
                instructions.push((i.address() as u32, i.to_string()));
            }

            return Ok(Command::Response(DebugResponse::Code {
                pc: pc_val, 
                instructions: instructions,
            }));
        } else {
            warn!("Core is not halted, status: {:?}", status);
            return Err(anyhow!("Core must be halted"));
        }
    }


    fn clear_all_breakpoints_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        core.clear_all_hw_breakpoints()?;
        self.breakpoints = HashMap::new();
    
        info!("All breakpoints cleared");
    
        Ok(Command::Response(DebugResponse::ClearAllBreakpoints))
    }


    fn clear_breakpoint_command(&mut self,
                                address: u32
                                ) -> Result<Command>
    {
        let mut core = self.session.core(0)?;

        match self.breakpoints.remove(&address) {
            Some(_bkpt) => {
                core.clear_hw_breakpoint(address)?; 
                info!("Breakpoint cleared from: 0x{:08x}", address);
                Ok(Command::Response(DebugResponse::ClearBreakpoint))
            },
            None => {
                core.clear_hw_breakpoint(address)?; 
                Err(anyhow!("Can't remove hardware breakpoint at {}", address))
            },
        }
    }


    fn set_breakpoint_command(&mut self, mut address: u32, source_file: Option<String>) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        address = match source_file {
            Some(path) => find_breakpoint_location(self.debugger.dwarf, &self.cwd, &path, address as u64, None)?.expect("Could not file location form source file line number") as u32,
            None => address,
        };

        let num_bkpt = self.breakpoints.len() as u32;
        let tot_bkpt = core.get_available_breakpoint_units()?;

        if num_bkpt < tot_bkpt {
            core.set_hw_breakpoint(address)?;

            let breakpoint = Breakpoint {
                id: Some(address as i64),
                verified: true,
                message: None,
                source: None,   // TODO
                line: None,     // TODO
                column: None,   // TODO
                end_line: None,
                end_column: None,
            };
            let _bkpt = self.breakpoints.insert(address, breakpoint);
    
            info!("Breakpoint set at: 0x{:08x}", address);
            return Ok(Command::Response(DebugResponse::SetBreakpoint));

        } else {
            return Err(anyhow!("All hardware breakpoints are already set"));
        } 
    }


    fn registers_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let register_file = core.registers();
   
        let mut registers = vec!();
        for register in register_file.registers() {
            let value = core.read_core_reg(register)?;
   
            registers.push((format!("{}", register.name()), value));
        }
    
        Ok(Command::Response(DebugResponse::Registers {
            registers: registers,
        }))
    }


    fn variable_command(&mut self,
                     name: &str
                     ) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;

        match status.is_halted() {
            true => {
                let pc  = core.read_core_reg(core.registers().program_counter())?; 
                let unit = get_current_unit(&self.debugger.dwarf, pc)?; 

                let value = debugger::find_variable(self.debugger.dwarf, &mut core, &unit, pc, name)?;

                Ok(Command::Response(DebugResponse::Variable {
                    name: name.to_string(),
                    value: format!("{}", value),
                }))
            },
            false => Err(anyhow!("Core must be halted")),
        }
    }


    fn stack_trace_command(&mut self) -> Result<Command>
    { 
        let mut core = self.session.core(0)?;
        let stack_trace = get_current_stacktrace(self.debugger.dwarf,
                                                 self.debugger.debug_frame,
                                                 &mut core,
                                                 &self.cwd)?;
        Ok(Command::Response(DebugResponse::StackTrace {
            stack_trace: stack_trace,
        }))
    }


    fn read_command(&mut self, address: u32, byte_size: usize) -> Result<Command> {
        let mut core = self.session.core(0)?;
        let mut buff: Vec<u8> = vec![0; byte_size];
        core.read_8(address, &mut buff)?; 

        Ok(Command::Response(DebugResponse::Read {
            address: address,
            value: buff,
        }))
    }


    fn reset_command(&mut self, reset_and_halt: bool) -> Result<Command>
    {
        let mut core = self.session.core(0)?;

        if reset_and_halt {
            core.reset_and_halt(std::time::Duration::from_millis(10)).context("Failed to reset and halt the core")?;
        } else {
            core.reset().context("Failed to reset the core")?;
        }

        self.running = true;

        Ok(Command::Response(DebugResponse::Reset))
    }


    fn flash_command(&mut self, reset_and_halt: bool) -> Result<Command>
    {
        download_file(&mut self.session, &self.file_path, Format::Elf).context("Failed to flash target")?;

        let mut core = self.session.core(0)?;

        if reset_and_halt {
            core.reset_and_halt(std::time::Duration::from_millis(10)).context("Failed to reset and halt the core")?;

        } else {
            core.reset().context("Failed to reset the core")?;
        }

        self.running = true;

        Ok(Command::Response(DebugResponse::Flash))
    }


    fn halt_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;

        if status.is_halted() {
            warn!("Core is already halted, status: {:?}", status);
            return Err(anyhow!("Core is already halted"));
        } else {
            let cpu_info = core.halt(Duration::from_millis(100))?;
            info!("Core halted at pc = 0x{:08x}", cpu_info.pc);
        };

        Ok(Command::Response(DebugResponse::Halt))
    }


    fn status_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;
        let mut pc = None;

        if status.is_halted() {
            pc = Some(core.read_core_reg(core.registers().program_counter())?);
        }

        Ok(Command::Response(DebugResponse::Status {
            status: status,
            pc: pc,
        }))
    }


    fn step_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;

        if status.is_halted() {
            let pc = continue_fix(&mut core, &self.breakpoints)?;
            self.running = true;
            info!("Stopped at pc = 0x{:08x}", pc);

            return Ok(Command::Response(DebugResponse::Step));
        }


        Ok(Command::Response(DebugResponse::Error {
            message: "Can only step when core is halted".to_owned(),
        }))
    }


    fn continue_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;

        if status.is_halted() {
            let _pc = continue_fix(&mut core, &self.breakpoints)?;
            core.run()?;
            self.running = true;
        }

        info!("Core status: {:?}", core.status()?);

        Ok(Command::Response(DebugResponse::Continue))
    }


    fn set_breakpoints_command(&mut self,
                               source_file: String,
                               source_breakpoints: Vec<SourceBreakpoint>
                               ) -> Result<Command>
    {
        // Clear all existing breakpoints
        let mut core = self.session.core(0)?;
        core.clear_all_hw_breakpoints()?;
        self.breakpoints = HashMap::new();

        let mut breakpoints= vec!();
        for bkpt in source_breakpoints {
            let breakpoint = match find_breakpoint_location(self.debugger.dwarf, &self.cwd, &source_file, bkpt.line as u64, bkpt.column.map(|c| c as u64))? {
                Some(address) => {
                    let mut breakpoint = Breakpoint {
                        id: Some(address as i64),
                        verified: true,
                        message: None,
                        source: None,
                        line: Some(bkpt.line),
                        column: bkpt.column,
                        end_line: None,
                        end_column: None,
                    };

                    // Set breakpoint
                    if self.breakpoints.len() < core.get_available_breakpoint_units()? as usize {
                        self.breakpoints.insert(address as u32, breakpoint.clone());
                        core.set_hw_breakpoint(address as u32)?;
                    } else {
                        breakpoint.verified = false;
                    }

                    breakpoint
                },
                None => {
                    Breakpoint {
                        id: None,
                        verified: false,
                        message: None,
                        source: None,
                        line: Some(bkpt.line),
                        column: bkpt.column,
                        end_line: None,
                        end_column: None,
                    }
                },
            };

            breakpoints.push(breakpoint);
        }

        Ok(Command::Response(DebugResponse::SetBreakpoints {
            breakpoints: breakpoints,
        }))
    }
}


fn continue_fix(core: &mut probe_rs::Core, breakpoints: &HashMap<u32, Breakpoint>) -> Result<u32, probe_rs::Error>
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

                        return Ok(step_pc);
                    } else {
                        match breakpoints.get(&pc_val) {
                            Some(_bkpt) => {
                                core.clear_hw_breakpoint(pc_val)?;
                                let pc = core.step()?.pc;
                                core.set_hw_breakpoint(pc_val)?;
                                return Ok(pc);
                            },
                            None => (),
                        };
                    }
                },
                _ => (),
            };
        },
        _ => (),
    };

    Ok(core.step()?.pc)
}

