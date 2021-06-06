mod config;


use config::Config;

use super::commands_temp::{
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

use std::sync::mpsc::{Sender, Receiver, TryRecvError};

use capstone::arch::BuildsCapstone;

use super::debugger::Debugger;

use super::{
    read_dwarf,
    attach_probe,
    get_current_unit,
};

use std::path::PathBuf;

use gimli::Reader;


use probe_rs::{
    MemoryInterface,
    CoreInformation,
    CoreStatus,
};

use log::{
    info,
    debug,
    warn,
};

use std::time::{Instant, Duration};

use probe_rs::flashing::{
    Format,
    download_file,
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
               mut sender: Sender<Command>,
               mut reciver: Receiver<DebugRequest>
              ) -> Result<()>
    {
        loop {
            let request = reciver.recv()?;
            let (exit, response) = self.handle_request(&mut sender, &mut reciver, request)?;
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

pub fn init(sender: &mut Sender<Command>,
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
        file_path: file_path,
        check_time: Instant::now(),
        running: true,
    };

    debug.run(sender, reciver, request)
}



struct DebugServer<'a, R: Reader<Offset = usize>> {
    debugger:   Debugger<'a, R>,
    session:    probe_rs::Session,
    capstone:   capstone::Capstone,
    breakpoints: Vec<u32>,
    file_path:  PathBuf,
    check_time: Instant, 
    running: bool,
}

impl<'a, R: Reader<Offset = usize>> DebugServer<'a, R> {

    pub fn run(&mut self,
               sender: &mut Sender<Command>,
               reciver: &mut Receiver<DebugRequest>,
               mut request: DebugRequest
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
                        Command::Request(req) => return Ok(req),
                        Command::Response(res) => sender.send(Command::Response(res))?,
                        _ => unimplemented!(),
                    };
                },
                Err(err) => {
                    match err {
                        TryRecvError::Empty => self.check_halted(sender)?,
                        TryRecvError::Disconnected => return Err(anyhow!("{:?}", err)),
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
            println!("Core halted at address {:#010x}", pc);

            sender.send(Command::Event(DebugEvent::Halted{ pc: pc, reason: reason }))?; 
        }

        Ok(())
    }


    fn handle_request(&mut self,
                      request: DebugRequest
                      ) -> Result<Command>
    {
        match request {
            DebugRequest::Stack => self.stack_command(),
            DebugRequest::Code => self.code_command(),
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

            _ => Ok(Command::Request(request)),
        }
    }


    fn stack_command(&mut self) -> Result<Command>
    { 
        let mut core = self.session.core(0)?;
        let status = core.status()?;
        if status.is_halted() {
            let sp_reg: u16 = probe_rs::CoreRegisterAddress::from(core.registers().stack_pointer()).0;
    
            let sf = core.read_core_reg(7)?; // reg 7 seams to be the base stack address.
            let sp = core.read_core_reg(sp_reg)?;
    //        println!("sf: {:?}, sp: {:?}", sf, sp);
            if sf < sp {
                // The previous stack pointer is less then current.
                // This happens when there is no stack.
                println!("Stack is empty");
                return Ok(Command::Response(DebugResponse::Code));
            }
            let length = (((sf - sp) + 4 - 1)/4) as usize;
            let mut stack = vec![0u32; length];
            core.read_32(sp, &mut stack);
        
            println!("Current stack value:");
            for i in 0..stack.len() {
                println!("\t{:#010x}: {:#010x}", sp as usize + i*4, stack[i]);
            }
        } else {
            println!("Core must be halted, status: {:?}", status);
        }
    
        Ok(Command::Response(DebugResponse::Stack))
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

            for i in insns.iter() {
                let mut spacer = "  ";
                if i.address() == pc_val as u64 {
                    spacer = "> ";
                }
                println!("{}{}", spacer, i);
            }

        } else {
            warn!("Core is not halted, status: {:?}", status);
            println!("Core is not halted, status: {:?}", status);
        }

        Ok(Command::Response(DebugResponse::Code))
    }


    fn clear_all_breakpoints_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        for bkpt in &self.debugger.breakpoints {
            core.clear_hw_breakpoint(*bkpt)?;
        }
        self.debugger.breakpoints = vec!();
    
        info!("All breakpoints cleard");
        println!("All breakpoints cleared");
    
        Ok(Command::Response(DebugResponse::ClearAllBreakpoints))
    }


    fn clear_breakpoint_command(&mut self,
                                address: u32
                                ) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
    
        for i in 0..self.debugger.breakpoints.len() {
            if address == self.debugger.breakpoints[i] {
                core.clear_hw_breakpoint(address)?;
                self.debugger.breakpoints.remove(i);
    
                info!("Breakpoint cleared from: 0x{:08x}", address);
                println!("Breakpoint cleared from: 0x{:08x}", address);
                break;
            }
        }
    
    
        Ok(Command::Response(DebugResponse::ClearBreakpoint))
    }


    fn set_breakpoint_command(&mut self, mut address: u32, source_file: Option<String>) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        address = match source_file {
            Some(path) => self.debugger.find_location(&path, address as u64, None)?.expect("Could not file location form  source file line number") as u32,
            None => address,
        };

        let num_bkpt = self.debugger.breakpoints.len() as u32;
        let tot_bkpt = core.get_available_breakpoint_units()?;

        if num_bkpt < tot_bkpt {
            core.set_hw_breakpoint(address)?;
            self.debugger.breakpoints.push(address);
    
            info!("Breakpoint set at: 0x{:08x}", address);
            println!("Breakpoint set at: 0x{:08x}", address);
        } else {
            println!("All hw breakpoints are already set");
        }
 
        Ok(Command::Response(DebugResponse::SetBreakpoint))
    }


    fn registers_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let register_file = core.registers();
    
        for register in register_file.registers() {
            let value = core.read_core_reg(register)?;
    
            println!("{}:\t{:#010x}", register.name(), value)
        }
    
        Ok(Command::Response(DebugResponse::Registers))
    }


    fn variable_command(&mut self,
                     name: &str
                     ) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;
    
        if status.is_halted() {
            let pc  = core.read_core_reg(core.registers().program_counter())?;
    
            let unit = get_current_unit(&self.debugger.dwarf, pc)?;
            //println!("{:?}", unit.name.unwrap().to_string());
            
            let value = self.debugger.find_variable(&mut core, &unit, pc, name);
            
            match value {
                Ok(val)     => println!("{} = {}", name, val),
                Err(_err)   => println!("Could not find {}", name),
            };
        } else {
            println!("CPU must be halted to run this command");
            println!("Status: {:?}", &status);
        }
    
        Ok(Command::Response(DebugResponse::Variable))
    }


    fn stack_trace_command(&mut self) -> Result<Command>
    { 
        let mut core = self.session.core(0)?;
        println!("result: {:#?}", self.debugger.get_current_stacktrace(&mut core)?);
        Ok(Command::Response(DebugResponse::StackTrace))
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
            match core.reset_and_halt(std::time::Duration::from_millis(10)).context("Failed to reset and halt the core") {
                Ok(_) => (),
                Err(err) => {
                    return Ok(Command::Response(DebugResponse::Reset {
                        message: Some(format!("{:?}", err)),
                    }));
                },
            };

        } else {
            match core.reset().context("Failed to reset the core") {
                Ok(_) => (),
                Err(err) => {
                    return Ok(Command::Response(DebugResponse::Reset {
                        message: Some(format!("{:?}", err)),
                    }));
                },
            };
        }

        self.running = true;

        Ok(Command::Response(DebugResponse::Reset {
            message: None,
        }))
    }


    fn flash_command(&mut self, reset_and_halt: bool) -> Result<Command>
    {
        match download_file(&mut self.session, &self.file_path, Format::Elf).context("Failed to flash target") {
            Ok(_) => (),
            Err(err) => {
                return Ok(Command::Response(DebugResponse::Flash {
                    message: Some(format!("{:?}", err)),
                }));
            },
        };

        let mut core = self.session.core(0)?;

        if reset_and_halt {
            match core.reset_and_halt(std::time::Duration::from_millis(10)).context("Failed to reset and halt the core") {
                Ok(_) => (),
                Err(err) => {
                    return Ok(Command::Response(DebugResponse::Flash {
                        message: Some(format!("{:?}", err)),
                    }));
                },
            };

        } else {
            match core.reset().context("Failed to reset the core") {
                Ok(_) => (),
                Err(err) => {
                    return Ok(Command::Response(DebugResponse::Flash {
                        message: Some(format!("{:?}", err)),
                    }));
                },
            };
        }

        self.running = true;

        Ok(Command::Response(DebugResponse::Flash {
            message: None,
        }))
    }


    fn halt_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;
    
        let pc = if status.is_halted() {
            warn!("Core is already halted, status: {:?}", status);
            println!("Core is already halted, status: {:?}", status);
            core.read_core_reg(core.registers().program_counter())?

        } else {
            let cpu_info = core.halt(Duration::from_millis(100))?;
            info!("Core halted at pc = 0x{:08x}", cpu_info.pc);
    
            let mut code = [0u8; 16 * 2];
    
            core.read_8(cpu_info.pc, &mut code)?;
    
    
            let insns = self.capstone.disasm_all(&code, cpu_info.pc as u64)
                .expect("Failed to disassemble");
            
            for i in insns.iter() {
                let mut spacer = "  ";
                if i.address() == cpu_info.pc as u64 {
                    spacer = "> ";
                }
                println!("{}{}", spacer, i);
            }
        
            cpu_info.pc
        };
        
        Ok(Command::Response(DebugResponse::Halt { pc: pc }))
    }


    fn status_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;
    
        println!("Status: {:?}", &status);
    
        if status.is_halted() {
            let pc = core.read_core_reg(core.registers().program_counter())?;
            println!("Core halted at address {:#010x}", pc);
        }
    
        Ok(Command::Response(DebugResponse::Status))
    }


    fn step_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;
    
        if status.is_halted() {
            let cpu_info = continue_fix(&mut core, &self.breakpoints)?;
            info!("Stept to pc = 0x{:08x}", cpu_info.pc);
    
            println!("Core stopped at address 0x{:08x}", cpu_info.pc);

            return Ok(Command::Response(DebugResponse::Step { pc: Some(cpu_info.pc) }));
        }
        
        
        Ok(Command::Response(DebugResponse::Step { pc: None }))
    }


    fn continue_command(&mut self) -> Result<Command>
    {
        let mut core = self.session.core(0)?;
        let status = core.status()?;
   
        if status.is_halted() {
            let _cpu_info = continue_fix(&mut core, &self.breakpoints)?;
            core.run()?;
            self.running = true;
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

