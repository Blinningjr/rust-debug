use crossbeam_channel::{ 
    unbounded,
    Sender,
    Receiver,
};

use std::thread;

use anyhow::{
    Result,
};


use super::{
    debug_handler::{
        DebugHandler,
    },
    commands::{
        Command,
        debug_response::DebugResponse,
        debug_request::DebugRequest,
        debug_event::DebugEvent,
        commands::Commands,
    },
};

use probe_rs::{
    CoreStatus,
    HaltReason,
};

use rustyline::Editor;

use crate::debugger::stack_frame::StackFrame;

use debugserver_types::Breakpoint;



pub fn debug_mode(opt: super::Opt) -> Result<()> {
    let (sender_to_reader, reader_receiver): (Sender<bool>, Receiver<bool>) = unbounded();
    let (sender_to_cli, cli_receiver): (Sender<Command>, Receiver<Command>) = unbounded();
    let (sender_to_debugger, debug_receiver): (Sender<DebugRequest>, Receiver<DebugRequest>) = unbounded();

    let debug_sender = sender_to_cli.clone();

    let debugger_th = thread::spawn(move || {
        let mut debugger = DebugHandler::new(Some(opt));
        debugger.run(debug_sender, debug_receiver).unwrap();
    });

    let reader_th = thread::spawn(move || {
        command_reader(sender_to_cli, reader_receiver).unwrap();
    });
    

    let mut cli = Cli::new(sender_to_debugger, cli_receiver, sender_to_reader);
    cli.run()?;

    debugger_th.join().expect("oops! the child thread panicked");
    reader_th.join().expect("oops! the child thread panicked");

    Ok(())
}


fn command_reader(sender: Sender<Command>,
                  receiver: Receiver<bool>
                  ) -> Result <()>
{
    let mut rl = Editor::<()>::new(); 
    let cmd_parser = Commands::new();

    loop {
        let readline = rl.readline(">> ");

        match readline {
            Ok(line) => {
                let history_entry: &str = line.as_ref();
                rl.add_history_entry(history_entry);

                if let Some(help_string) = cmd_parser.check_if_help(history_entry) {
                    println!("{}", help_string);
                    continue;
                } else if &line == "" {
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


struct Cli {
    debug_sender:   Sender<DebugRequest>,
    receiver:       Receiver<Command>,
    cli_sender:     Sender<bool>,
}

impl Cli {
    pub fn new(debug_sender: Sender<DebugRequest>,
               receiver: Receiver<Command>,
               cli_sender: Sender<bool>
               ) -> Cli
    {
        Cli {
            debug_sender: debug_sender,
            receiver: receiver,
            cli_sender: cli_sender,
        }
    }


    pub fn run(&mut self) -> Result<()> {
        loop {
            let command = self.receiver.recv()?;
            match self.handle_command(command)? {
                Some(exit) => {
                    self.cli_sender.send(exit)?;
                    if exit {
                        return Ok(());
                    } 
                },
                None => (),
            };
        }
    }


    fn handle_command(&mut self, command: Command) -> Result<Option<bool>> {
        match command {
            Command::Request(req) => self.debug_sender.send(req)?,
            Command::Response(res) => return Ok(Some(self.handle_response(res)?)),
            Command::Event(event) => self.handle_event(event),
        };

        Ok(None)
    }


    fn handle_event(&mut self, event: DebugEvent) {
        match event {
            DebugEvent::Halted { pc, reason, hit_breakpoint_ids: _ } => self.handle_halted_event(pc, reason),
        };
    }
    

    fn handle_halted_event(&self, pc: u32, reason: HaltReason) {
        println!("Core halted a pc: {}, because: {:?}", pc, reason);
    }


    fn handle_response(&mut self, response: DebugResponse) -> Result<bool> {
        //println!("{:?}", response);
        match response {
            DebugResponse::Exit => return Ok(true),

            DebugResponse::Attach => self.handle_attach_response(),
            DebugResponse::Status { status, pc } => self.handle_status_response(status, pc),
            DebugResponse::Continue => self.handle_continue_response(),
            DebugResponse::Step => self.handle_step_response(),
            DebugResponse::Halt => self.handle_halt_response(),
            DebugResponse::SetBinary => self.handle_set_binary_response(),
            DebugResponse::Flash => self.handle_flash_response(),
            DebugResponse::Reset => self.handle_reset_response(),
            DebugResponse::Read { address, value } => self.handle_read_response(address, value),
            DebugResponse::StackTrace { stack_trace } => self.handle_stack_trace_response(stack_trace),
            DebugResponse::SetProbeNumber => self.handle_set_probe_number_response(),
            DebugResponse::SetChip => self.handle_set_chip_response(),
            DebugResponse::Variable { name, value } => self.handle_variable_response(name, value),
            DebugResponse::Registers { registers } => self.handle_registers_response(registers),
            DebugResponse::SetBreakpoint => self.handle_set_breakpoint_response(),
            DebugResponse::SetBreakpoints { breakpoints } => self.handle_set_breakpoints_response(breakpoints),
            DebugResponse::ClearBreakpoint => self.handle_clear_breakpoint_response(),
            DebugResponse::ClearAllBreakpoints => self.handle_clear_all_breakpoints_response(),
            DebugResponse::Code { pc, instructions } => self.handle_code_response(pc, instructions),
            DebugResponse::Stack { stack_pointer, stack } => self.handle_stack_response(stack_pointer, stack),
            DebugResponse::Error { message } => self.handle_error_response(message),
            DebugResponse::SetCWD => self.handle_set_cwd_response(),
        };
        
        Ok(false)
    }


    fn handle_attach_response(&self) {
        println!("Debugger attached successfully");
    }


    fn handle_status_response(&self, status: CoreStatus, pc: Option<u32>) {
        println!("Status: {:?}", &status);
        if status.is_halted() && pc.is_some() {
            println!("Core halted at address {:#010x}", pc.unwrap());
        }
    }


    fn handle_continue_response(&self) {
        println!("Core is running");
    }


    fn handle_step_response(&self) {
        return (); 
    }


    fn handle_halt_response(&self) {
        return ();
    }


    fn handle_set_binary_response(&self) {
        println!("Binary file path set "); 
    }


    fn handle_flash_response(&self) {
        println!("Flash successful");
    }


    fn handle_reset_response(&self) {
        println!("Target reset");
    }


    fn handle_read_response(&self, address: u32, value: Vec<u8>) { // TODO
        let mut value_string = "".to_owned();

        let address_string = format!("0x{:08x}:", address);
        let mut spacer = "".to_string();
        for _ in 0..address_string.len() {
            spacer.push(' ');
        }

        let mut i = 0;
        for val in value { // TODO: print in right order.
            if i == 4 {
                value_string = format!("{}\n\t{} {:02x}", value_string, spacer, val);
                i = 0;
            } else {
                value_string = format!("{} {:02x}", value_string, val);
            }
            i += 1;
        }
        println!("\t{}{}", address_string, value_string);
    }


    fn handle_stack_trace_response(&self, stack_trace: Vec<StackFrame>) {
        println!("\nStack Trace:");
        for sf in &stack_trace {
            self.print_stack_frame(sf);
        }
    }

    fn print_stack_frame(&self, stack_frame: &StackFrame) {
        println!("\tName: {}", stack_frame.name);
        println!("\tline: {:?}, column: {:?}, pc: {:?}",
                 match stack_frame.source.line {
                     Some(l) => l.to_string(),
                     None => "<unknown>".to_string(),
                 },
                 match stack_frame.source.column {
                     Some(l) => l.to_string(),
                     None => "<unknown>".to_string(),
                 },
                 stack_frame.call_frame.code_location);
        println!("\tfile: {}, directory: {}",
                 match &stack_frame.source.file {
                     Some(val) => val,
                     None => "<unknown>",
                 },
                 match &stack_frame.source.file {
                     Some(val) => val,
                     None => "<unknown>",
                 });

        for var in &stack_frame.variables {
            println!("\t{:?} = {:?}",
                     var.name,
                     var.value);
        }
        println!("");
    }


    fn handle_set_probe_number_response(&self) {
        println!("Probe number set "); 
    }


    fn handle_set_chip_response(&self) {
        println!("Chip set"); 
    }


    fn handle_variable_response(&self, name: String, value: String) {
        println!("{} = {}", name, value);
    }


    fn handle_registers_response(&self, registers: Vec<(String, u32)>) {
        println!("Registers:");
        for (name, value) in &registers {
            println!("\t{}: {:#010x}", name, value)
        }
    }


    fn handle_set_breakpoint_response(&self) {
        println!("Breakpoint set");
    }


    fn handle_set_breakpoints_response(&self, _breakpoints: Vec<Breakpoint>) {
        unreachable!();
    }


    fn handle_clear_breakpoint_response(&self) {
        println!("Breakpoint cleared");
    }


    fn handle_clear_all_breakpoints_response(&self) {
        println!("All hardware breakpoints cleared");
    }


    fn handle_code_response(&self, pc: u32, instructions: Vec<(u32, String)>) {
        println!("Assembly Code");
        for (address, asm) in instructions {
            let mut spacer = "  ";
            if address == pc {
                spacer = "> ";
            }
            println!("{}{}", spacer, asm);
        }
    }


    fn handle_stack_response(&self, stack_pointer: u32, stack: Vec<u32>) {
        println!("Current stack value:");
        for i in 0..stack.len() {
            println!("\t{:#010x}: {:#010x}", stack_pointer as usize + i*4, stack[i]);
        }
    }


    fn handle_error_response(&self, message: String) {
        println!("Error: {}", message);
    }


    fn handle_set_cwd_response(&self) {
        println!("Current work directory set");
    }
}

