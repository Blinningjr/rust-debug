use anyhow::{
    Result,
    anyhow,
};

use serde_json::map::Map;
use serde_json::value::Value;
use serde_json::Number;
use serde_json::{
    json,
    to_vec,
};

use debugserver_types::{
    Response,
    Request, 

    DisconnectArguments,
    ContinueArguments,
    NextArguments,
    PauseArguments,
    StackTraceArguments,
};


struct CommandInfo {
    pub name:           &'static str,
    pub description:    &'static str,
    pub parser: fn(args: &[&str]) -> Result<Request>,
}


pub struct CommandParser {
    seq:        i64,
    commands:   Vec<CommandInfo>,
}

impl CommandParser {
    pub fn new() -> CommandParser {
        CommandParser {
            seq: 0,
            commands: vec!(
                //CommandInfo { // TODO: Is this the same as DAP Reset
                //    name: "reset",
                //    description: "Reset the program",
                //    parser: |_args| {
                //        Ok(NewCommand::Debug(DebugCommand::Reset))
                //    },
                //},


                // Dap commands
                CommandInfo {
                    name: "exit",
                    description: "Exit debugger",
                    parser: |_args| {
                        let arguments = DisconnectArguments {
                            restart: Some(false),
                            terminate_debuggee: None,
                        };

                        Ok(Request {
                            seq: 0,
                            type_: "request".to_owned(),
                            command: "disconnect".to_owned(),
                            arguments: Some(json!(arguments)),
                        })
                    },
                },
                CommandInfo {
                    name: "continue",
                    description: "Continue the program",
                    parser: |_args| {
                        let arguments = ContinueArguments {
                            thread_id: 0,
                        };

                        Ok(Request {
                            seq: 0,
                            type_: "request".to_owned(),
                            command: "continue".to_owned(),
                            arguments: Some(json!(arguments)),
                        })
                    },
                },
                CommandInfo {
                    name: "next",
                    description: "Step one instruction",
                    parser: |_args| {
                        let arguments = NextArguments {
                            thread_id: 0,
                        };

                        Ok(Request {
                            seq: 0,
                            type_: "request".to_owned(),
                            command: "next".to_owned(),
                            arguments: Some(json!(arguments)),
                        })
                    },
                },
                CommandInfo {
                    name: "pause",
                    description: "Halt the core",
                    parser: |_args| {
                        let arguments = PauseArguments {
                                thread_id: 0,
                        };

                        Ok(Request {
                            seq: 0,
                            type_: "request".to_owned(),
                            command: "pause".to_owned(),
                            arguments: Some(json!(arguments)),
                        })
                    },
                },
                //CommandInfo { // TODO
                //    name: "read",
                //    description: "Read memory at an address of a certain byte size(default: 4 bytes)",
                //    parser: |args| {
                //        if args.len() > 0 {
                //            let mut map = Map::new();

                //            let address = Number::from(parse_u32_from_str(args[0])?);
                //            map.insert("address".to_owned(), Value::Number(address));

                //            let byte_size = Number::from(if args.len() > 1 {
                //                parse_u32_from_str(args[1])?
                //            } else {
                //                4
                //            });
                //            map.insert("byte_size".to_owned(), Value::Number(byte_size));

                //            return Ok(Request {
                //                seq: 0,
                //                type_: "request".to_owned(),
                //                command: "readMemory".to_owned(), // TODO:
                //                arguments: Some(Value::Object(map)),
                //            });
                //        }
                //        
                //        Err(anyhow!("Requires a number as a argument"))
                //    },
                //},
                CommandInfo {
                    name: "stack-trace",
                    description: "Prints the current stack trace",
                    parser: |_args| {
                        let arguments = StackTraceArguments {
                            thread_id: 0,
                            levels: None,
                            start_frame: None,
                            format: None,
                        };
                        Ok(Request {
                            seq: 0,
                            type_: "request".to_owned(),
                            command: "stackTrace".to_owned(), // TODO:
                            arguments: Some(json!(arguments)),
                        })
                    },
                },


                // Non Dap commands
                //CommandInfo {
                //    name: "set-binary",
                //    description: "Set the binary file to debug",
                //    parser: |args| {
                //        if args.len() > 0 {
                //            return Ok(Request {
                //                seq: 0,
                //                type_: "request".to_owned(),
                //                command: "setbinary".to_owned(),
                //                arguments: Some(Value::String(args[0].to_string())),
                //            });
                //        }
                //        Err(anyhow!("Requires a path as a argument"))
                //    },
                //},
                //CommandInfo {
                //    name: "set-probe-number",
                //    description: "Set the probe number to use",
                //    parser: |args| {
                //        if args.len() > 0 {
                //            let number = Number::from(args[0].parse::<u32>()?);
                //            return Ok(Request {
                //                seq: 0,
                //                type_: "request".to_owned(),
                //                command: "setprobenumber".to_owned(),
                //                arguments: Some(Value::Number(number)),
                //            });
                //        }
                //        Err(anyhow!("Requires a number as a argument"))
                //    },
                //},
                //CommandInfo {
                //    name: "set-chip",
                //    description: "Set the chip to use",
                //    parser: |args| {
                //        if args.len() > 0 {
                //            return Ok(Request {
                //                seq: 0,
                //                type_: "request".to_owned(),
                //                command: "setchip".to_owned(),
                //                arguments: Some(Value::String(args[0].to_string())),
                //            });
                //        }
                //        Err(anyhow!("Requires a String as a argument"))
                //    },
                //},
                //CommandInfo {
                //    name: "status",
                //    description: "Get the core status",
                //    parser: |_args| {
                //        return Ok(Request {
                //            seq: 0,
                //            type_: "request".to_owned(),
                //            command: "status".to_owned(),
                //            arguments: Some(Value::Bool(true)), // NOTE: True to indicate that this should be printed.
                //        });
                //    },
                //},
                //CommandInfo {
                //    name: "variable",
                //    description: "Print a variables value",
                //    parser: |args| {
                //        if args.len() > 0 {
                //            return Ok(Request {
                //                seq: 0,
                //                type_: "request".to_owned(),
                //                command: "variable".to_owned(),
                //                arguments: Some(Value::String(args[0].to_string())),
                //            });
                //        }
                //        Err(anyhow!("Requires a String as a argument"))
                //    },
                //},
                //CommandInfo {
                //    name: "registers",
                //    description: "Print all the values of the registers",
                //    parser: |_args| {
                //        Ok(Request {
                //            seq: 0,
                //            type_: "request".to_owned(),
                //            command: "registers".to_owned(),
                //            arguments: None,
                //        })
                //    },
                //},
                //CommandInfo {
                //    name: "set-breakpoint",
                //    description: "Set a hardware breakpoint at an address",
                //    parser: |args| {
                //        if args.len() > 0 {
                //            let address = Number::from(parse_u32_from_str(args[0])?);
                //            return Ok(Request {
                //                seq: 0,
                //                type_: "request".to_owned(),
                //                command: "setbreakpoint".to_owned(),
                //                arguments: Some(Value::Number(address)),
                //            });
                //        }
                //        Err(anyhow!("Requires a address as a argument"))
                //    },
                //},
                //CommandInfo {
                //    name: "clear-breakpoint",
                //    description: "Remobe a hardware breakpoint from an address",
                //    parser: |args| {
                //        if args.len() > 0 {
                //            let address = Number::from(parse_u32_from_str(args[0])?);
                //            return Ok(Request {
                //                seq: 0,
                //                type_: "request".to_owned(),
                //                command: "clearbreakpoint".to_owned(),
                //                arguments: Some(Value::Number(address)),
                //            });
                //        }
                //        
                //        Err(anyhow!("Requires a String as a argument"))
                //    },
                //},
                //CommandInfo { // TODO: setbreakpoint with zero arguments dose the same.
                //    name: "clear-all-breakpoints",
                //    description: "Removes all hardware breakpoints",
                //    parser: |_args| {
                //        Ok(Request {
                //            seq: 0,
                //            type_: "request".to_owned(),
                //            command: "clearallbreakpoints".to_owned(),
                //            arguments: None,
                //        })
                //    },
                //},


                // TODO
                //CommandInfo {
                //    name: "number-of-breakpoints",
                //    description: "Prints the number of active and total hardware breakpoints",
                //    parser: |_args| {
                //        Ok(NewCommand::Debug(DebugCommand::NumberOfBreakpoints))
                //    },
                //},
                //CommandInfo {
                //    name: "code",
                //    description: "Prints the assembly code in memory at the current program counter",
                //    parser: |_args| {
                //        Ok(NewCommand::Debug(DebugCommand::Code))
                //    },
                //},
                //CommandInfo {
                //    name: "stack",
                //    description: "Prints the current stack values",
                //    parser: |_args| {
                //        Ok(NewCommand::Debug(DebugCommand::Stack))
                //    },
                //},
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


    pub fn parse_command(&self, line: &str) -> Result<Request> {
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

