use super::{
    commands::{
        Command,
    },
    debugger::{
        Debugger,
    },
};

use probe_rs::{
    Probe,
    Core,
    Session,
};

use gimli::{
    Reader,
    Dwarf,
};

use rustyline::Editor;
use std::path::PathBuf;
use std::{borrow, env, fs};
use object::{Object, ObjectSection};

pub struct DebuggerCli<'a, R: Reader<Offset = usize>> {
    pub commands:   Vec<Command<R>>,
    pub debugger:   Debugger<'a, R>,
}

impl<'a, R: Reader<Offset = usize>> DebuggerCli<'a, R> {
    pub fn new(debugger: Debugger<'a, R>) -> Result<DebuggerCli<'a, R>, &'static str>
    {
        Ok(DebuggerCli {
            commands:   Command::init_commands(),
            debugger:   debugger,
        })
    }


    pub fn run(&mut self) -> Result<(), &'static str>
    {
        let mut rl = Editor::<()>::new();
    
        loop {
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    let history_entry: &str = line.as_ref();
                    rl.add_history_entry(history_entry);
                    
                    let exit_cli = self.handle_line(&line)?;
    
                    if exit_cli {
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

    
    pub fn handle_line(&mut self, line: &str) -> Result<bool, &'static str>
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
    
            let cmd = self.commands.iter().find(|c| c.name == command);
    
            if let Some(cmd) = cmd {
                let remaining_args: Vec<&str> = command_parts.collect();

                (cmd.function)(&mut self.debugger, &remaining_args)
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

