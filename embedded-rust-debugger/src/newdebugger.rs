
use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;

use std::io::{self, Write};

use super::{
    config::Config,
    newcommands::{
        NewCommand,
        ConfigCommand,
        NewResponse,
    },
    commands::{
        Command,
    },
    debugger::{
        Debugger,
    },
};


use gimli::{
    Reader,
};


use anyhow::{
    Result,
};


use rustyline::Editor;

use capstone::arch::BuildsCapstone;



pub struct DebugThread {
    config: Config,
}


impl DebugThread {
    pub fn new(opt: super::Opt) -> DebugThread {
        DebugThread {
            config: Config::new(opt),
        }
    }


    pub fn run(&mut self,
               sender: Sender<NewResponse>,
               reciver: Receiver<NewCommand>
              ) -> Result<()>
    {
        loop {
            let response = match reciver.recv()? {            
                NewCommand::Exit        => {
                    sender.send(NewResponse::Confirm)?;
                    return Ok(());
                },
                NewCommand::Config(cmd) => self.handle_command(cmd),
                _                       => NewResponse::Error(self.config.missing_config()),
            };

            sender.send(response)?;
        }
    }

    
    fn handle_command(&mut self, command: ConfigCommand) -> NewResponse {
        match command {
            ConfigCommand::SetBinary           (pb)    => self.config.bin = Some(pb),
            ConfigCommand::SetProbeNumber   (num)   => self.config.probe_num = num,
            ConfigCommand::SetChip          (chip)  => self.config.chip = Some(chip),
        };

        NewResponse::Confirm
    }


    fn start_debugger(& self,
                          sender: Sender<NewResponse>,
                          reciver: Receiver<NewCommand>
                          ) -> Result<bool>
    {
        Ok(false)
    }

}



pub struct NewDebugger<'a, R: Reader<Offset = usize>> {
    pub commands:   Vec<Command<R>>,
    pub debugger:   Debugger<'a, R>,
    pub session:    probe_rs::Session,
    pub capstone:   capstone::Capstone,
    pub halted:     bool,
}

impl<'a, R: Reader<Offset = usize>> NewDebugger<'a, R> {
    pub fn new(debugger: Debugger<'a, R>, session: probe_rs::Session) -> Result<NewDebugger<'a, R>>
    {
        let cs = capstone::Capstone::new() // TODO: Set the capstone base on the arch of the chip.
            .arm()
            .mode(capstone::arch::arm::ArchMode::Thumb)
            .build()
            .expect("Failed to create Capstone object");

        Ok(NewDebugger {
            commands:   Command::init_commands(),
            debugger:   debugger,
            session:    session,
            capstone:   cs,
            halted:     false,
        })
    }


    pub fn run(&mut self, sender: Sender<bool>, reciver: Receiver<String>, check_sender: Sender<bool>) -> Result<()>
    {
        loop {
            let line = reciver.recv()?;

            if line == "__checkhitbreakpoint__".to_string() {
                let mut core = match self.session.core(0) {
                    Ok(val) => val,
                    Err(_) => {
                        check_sender.send(false).unwrap();
                        continue;
                    },
                };
                let status = match core.status() {
                    Ok(val) => val,
                    Err(_) => {
                        check_sender.send(false).unwrap();
                        continue;
                    },
                };
                if status.is_halted() {
                    if !self.halted {
                        self.halted = true;
                        let pc = match core.read_core_reg(core.registers().program_counter()) {
                            Ok(val) => format!("{:#010x}", val),
                            Err(err) =>   format!("<{:?}>", err),
                        };
                        println!("\nStatus: {:?}", status);
                        println!("Core halted at address {}", pc); 
                        print!(">> "); 
                        io::stdout().flush();
                    }
                } else {
                    self.halted = false;
                }
                
                check_sender.send(false).unwrap();

                continue; 
            }
            
            let exit = match self.handle_line(&line) {
                Ok(val) => val,
                Err(_) => true,
            };

            sender.send(exit).unwrap();


            if exit {
                let _ = reciver.recv().unwrap();
                check_sender.send(true).unwrap();

                return Ok(());
            }
        }
    }

    
    pub fn handle_line(&mut self, line: &str) -> Result<bool>
    {
        let mut command_parts = line.split_whitespace();
    
        if let Some(command) = command_parts.next() {

            if command == "help" {
                println!("Available commands:");
                for cmd in &self.commands {
                    println!("\t- {}: {}", cmd.name, cmd.description);
                }
                return Ok(false);
            }
    
            let cmd = self.commands.iter().find(|c| c.name == command || c.short == command);
    
            if let Some(cmd) = cmd {
                let remaining_args: Vec<&str> = command_parts.collect();

                (cmd.function)(&mut self.debugger, &mut self.session, &mut self.capstone, &remaining_args)
            } else {
                println!("Unknown command '{}'", command);
                println!("Enter 'help' for a list of commands");
    
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
}


