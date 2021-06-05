use super::debug_request::DebugRequest;

use anyhow::{ Result, anyhow };

use std::path::PathBuf;



struct CommandInfo {
    pub name:           &'static str,
    pub description:    &'static str,
    pub parser: fn(args: &[&str]) -> Result<DebugRequest>,
}


pub struct Commands {
    commands:   Vec<CommandInfo>,
}

impl Commands {
    pub fn new() -> Commands {
        Commands {
            commands: vec!(
                CommandInfo {
                    name: "exit",
                    description: "Exit debugger",
                    parser: |_args| {
                        Ok(DebugRequest::Exit)
                    },
                },
                CommandInfo {
                    name: "continue",
                    description: "Continue the program",
                    parser: |_args| {
                        Ok(DebugRequest::Continue)
                    },
                },
                CommandInfo {
                    name: "halt",
                    description: "Halt the core",
                    parser: |_args| {
                        Ok(DebugRequest::Halt)
                    },
                },
                CommandInfo {
                    name: "set-binary",
                    description: "Set the binary file to debug",
                    parser: |args| {
                        if args.len() > 0 {
                            let path = PathBuf::from(args[0]);
                            return Ok(DebugRequest::SetBinary { path: path });
                        }
                        Err(anyhow!("Requires a path as a argument"))
                    },
                },
            ),
        }
    }


    pub fn parse_command(&self, line: &str) -> Result<DebugRequest> {
        let mut command_parts = line.split_whitespace();
        if let Some(command) = command_parts.next() {
            
            let cmd = self.commands.iter().find(|c| c.name == command);
            
            if let Some(cmd) = cmd {
                let remaining_args: Vec<&str> = command_parts.collect();

                return (cmd.parser)(&remaining_args);
            } else {
                return Err(anyhow!("Unknown command '{}'\n\tEnter 'help' for a list of commands", command));
            }
        }

        Err(anyhow!("Empty Command"))
    }


    pub fn check_if_help(&self, line: &str) -> Option<String> {
        let mut command_parts = line.split_whitespace();
        if let Some(command) = command_parts.next() {
            if command == "help" {
                let mut help_string = format!("Available commands:");
                for cmd in &self.commands {
                    help_string = format!("{}\n\t- {}: {}",
                                          help_string,
                                          cmd.name,
                                          cmd.description);
                }
                return Some(help_string);
            }
        }

        None
    }
}


fn parse_u32_from_str(s: &str) -> Result<u32> {
    if s.starts_with("0x") {
        let without_prefix = s.trim_start_matches("0x");
        return Ok(u32::from_str_radix(without_prefix, 16)?);
    } else {
        return Ok(u32::from_str_radix(s, 10)?); 
    };
}
