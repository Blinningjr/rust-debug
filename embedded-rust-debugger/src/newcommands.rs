use std::path::PathBuf;

use anyhow::{
    Result,
    anyhow,
};


#[derive(Debug, Clone)]
pub enum NewCommand {
    Debug(DebugCommand),
    Config(ConfigCommand),
    Exit,
}


#[derive(Debug, Clone)]
pub enum DebugCommand {
    Status,
    Print(String),
    Run,
    Step,
    Halt,
    Registers,
    Reset,
    Read { address: u32, byte_size: u32 },
    SetBreakpoint(u32),
    ClearBreakpoint(u32),
    ClearAllBreakpoints,
    NumberOfBreakpoints,
    Code,
    StackTrace,
    Stack,
}


#[derive(Debug, Clone)]
pub enum ConfigCommand {
    SetBinary(PathBuf),
    SetProbeNumber(u32),
    SetChip(String),
}



#[derive(Debug, Clone)]
pub enum NewResponse {
    Exited,
    Confirm,
    Error(String),
}


struct CommandInfo {
    pub name:           &'static str,
    pub description:    &'static str,
    pub parser: fn(args: &[&str]) -> Result<NewCommand>,
}


pub struct CommandParser {
    commands:   Vec<CommandInfo>,
}

impl CommandParser {
    pub fn new() -> CommandParser {
        CommandParser {
            commands: vec!(


                // General commands
                CommandInfo {
                    name: "Exit",
                    description: "Exit debugger",
                    parser: |_args| {
                        Ok(NewCommand::Exit)
                    },
                },


                // Configuration Commands
                CommandInfo {
                    name: "set-binary",
                    description: "Set the binary file to debug",
                    parser: |args| {
                        if args.len() > 0 {
                            let path = PathBuf::from(args[0]);
                            return Ok(NewCommand::Config(ConfigCommand::SetBinary(path)));
                        }
                        Err(anyhow!("Requires a path as a argument"))
                    },
                },
                CommandInfo {
                    name: "set-probe-number",
                    description: "Set the probe number to use",
                    parser: |args| {
                        if args.len() > 0 {
                            let number = args[0].parse::<u32>()?;
                            return Ok(NewCommand::Config(ConfigCommand::SetProbeNumber(number)));
                        }
                        Err(anyhow!("Requires a number as a argument"))
                    },
                },
                CommandInfo {
                    name: "set-chip",
                    description: "Set the chip to use",
                    parser: |args| {
                        if args.len() > 0 {
                            let chip = args[0].to_string();
                            return Ok(NewCommand::Config(ConfigCommand::SetChip(chip)));
                        }
                        Err(anyhow!("Requires a String as a argument"))
                    },
                },


                // Debugger Commands
                CommandInfo {
                    name: "status",
                    description: "Get the core status",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Status))
                    },
                },
                CommandInfo {
                    name: "print",
                    description: "Get the core status",
                    parser: |args| {
                        if args.len() > 0 {
                            let variable_name = args[0].to_string();
                            return Ok(NewCommand::Debug(DebugCommand::Print(variable_name)));
                        }
                        Err(anyhow!("Requires a String as a argument"))
                    },
                },
                CommandInfo {
                    name: "run",
                    description: "Run/Continue the program",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Run))
                    },
                },
                CommandInfo {
                    name: "step",
                    description: "Step one instruction",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Step))
                    },
                },
                CommandInfo {
                    name: "halt",
                    description: "Halt the core",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Halt))
                    },
                },
                CommandInfo {
                    name: "registers",
                    description: "Print all the values of the registers",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Registers))
                    },
                },
                CommandInfo {
                    name: "reset",
                    description: "Reset the program",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Reset))
                    },
                },
                CommandInfo {
                    name: "read",
                    description: "Read memory at an address of a certain byte size(default: 4 bytes)",
                    parser: |args| {
                        if args.len() > 0 {
                            let address = parse_u32_from_str(args[0])?;
                            let byte_size = if args.len() > 1 {
                                parse_u32_from_str(args[1])?
                            } else {
                                4
                            };
                            return Ok(NewCommand::Debug(DebugCommand::Read {
                                address: address,
                                byte_size: byte_size,
                            }));
                        }
                        
                        Err(anyhow!("Requires a number as a argument"))
                    },
                },
                CommandInfo {
                    name: "set-breakpoint",
                    description: "Set a hardware breakpoint at an address",
                    parser: |args| {
                        if args.len() > 0 {
                            let address = parse_u32_from_str(args[0])?;
                            return Ok(NewCommand::Debug(DebugCommand::SetBreakpoint(address)));
                        }
                        Err(anyhow!("Requires a String as a argument"))
                    },
                },
                CommandInfo {
                    name: "clear-breakpoint",
                    description: "Remobe a hardware breakpoint from an address",
                    parser: |args| {
                        if args.len() > 0 {
                            let address = parse_u32_from_str(args[0])?;
                            return Ok(NewCommand::Debug(DebugCommand::ClearBreakpoint(address)));
                        }
                        Err(anyhow!("Requires a String as a argument"))
                    },
                },
                CommandInfo {
                    name: "clear-all-breakpoints",
                    description: "Removes all hardware breakpoints",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::ClearAllBreakpoints))
                    },
                },
                CommandInfo {
                    name: "number-of-breakpoints",
                    description: "Prints the number of active and total hardware breakpoints",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::NumberOfBreakpoints))
                    },
                },
                CommandInfo {
                    name: "code",
                    description: "Prints the assembly code in memory at the current program counter",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Code))
                    },
                },
                CommandInfo {
                    name: "stack-trace",
                    description: "Prints the current stack trace",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::StackTrace))
                    },
                },
                CommandInfo {
                    name: "stack",
                    description: "Prints the current stack values",
                    parser: |_args| {
                        Ok(NewCommand::Debug(DebugCommand::Stack))
                    },
                },
            ),
        }
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


    pub fn parse_command(&self, line: &str) -> Result<NewCommand> {
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
}


fn parse_u32_from_str(s: &str) -> Result<u32> {
    if s.starts_with("0x") {
        let without_prefix = s.trim_start_matches("0x");
        return Ok(u32::from_str_radix(without_prefix, 16)?);
    } else {
        return Ok(u32::from_str_radix(s, 10)?); 
    };
}

