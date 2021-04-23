use super::{
    Core,
    Reader,
    get_current_unit,
    Debugger,
};


use anyhow::{
    Result,
};


use std::time::Duration;


use probe_rs::{
    MemoryInterface,
    CoreInformation,
};

use log::{
    info,
    debug,
    warn,
};



pub struct Command<R: Reader<Offset = usize>> {
    pub name:           &'static str,
    pub short:          &'static str,
    pub description:    &'static str,
    pub function:       fn(debugger: &mut Debugger<R>,
                           session: &mut probe_rs::Session,
                           cs: &mut capstone::Capstone,
                           args:    &[&str]
                           ) -> Result<bool>,
}


impl<R: Reader<Offset = usize>> Command<R> {
    pub fn init_commands() -> Vec<Command<R>> {
        vec!(
            Command {
                name:           "exit",
                short:          "e",
                description:    "Exit the debugger",
                function:       |_debugger, _session, _cs, _args| exit_command(),
            },
            Command {
                name:           "status",
                short:          "s",
                description:    "Show current status of CPU",
                function:       |debugger, session, _cs, _args| status_command(session),
            },
            Command {
                name:           "print",
                short:          "p",
                description:    "Evaluate variable",
                function:       |debugger, session, _cs, args| print_command(debugger, session, args),
            },
            Command {
                name:           "run",
                short:          "r",
                description:    "Resume execution of the CPU",
                function:       |debugger, session, _cs, _args| run_command(session, &debugger.breakpoints),
            },
            Command {
                name:           "step",
                short:          "sp",
                description:    "Step a single instruction",
                function:       |debugger, session, _cs, _args| step_command(session, &debugger.breakpoints, true),
            },
            Command {
                name:           "halt",
                short:          "h",
                description:    "Stop the CPU",
                function:       |debugger, session, cs, _args| halt_command(session, cs, true),
            },
            Command {
                name:           "registers",
                short:          "regs",
                description:    "Show CPU register values",
                function:       |debugger, session, _cs, _args| regs_command(session),
            },
            Command {
                name:           "reset",
                short:          "rt",
                description:    "Reset the CPU",
                function:       |debugger, session, _cs, _args| reset_command(session),
            },
            Command {
                name:           "read",
                short:          "rd",
                description:    "Read 32bit value from memory",
                function:       |debugger, session, _cs, args| read_command(session, args),
            },
            Command {
                name:           "set_breakpoint",
                short:          "bkpt",
                description:    "Set breakpoint at an address",
                function:       |debugger, session, _cs, args| set_breakpoint_command(debugger, session, args, true),
            },
            Command {
                name:           "clear_breakpoint",
                short:          "cbkpt",
                description:    "Clear breakpoint from an address",
                function:       |debugger, session, _cs, args| clear_breakpoint_command(debugger, session, args, true),
            },
            Command {
                name:           "clear_all_breakpoints",
                short:          "cabkpt",
                description:    "Clear all breakpoints",
                function:       |debugger, session, _cs, _args| clear_all_breakpoints_command(debugger, session, true),
            },
            Command {
                name:           "num_breakpoints",
                short:          "nbkpt",
                description:    "Get total number of hw breakpoints",
                function:       |debugger, session, _cs, _args| num_breakpoints_command(debugger, session),
            },
            Command {
                name:           "code",
                short:          "ce",
                description:    "Print first 16 lines of assembly code",
                function:       |debugger, session, cs, _args| code_command(session, cs),
            },
            Command {
                name:           "stacktrace",
                short:          "st",
                description:    "Print stack trace",
                function:       |debugger, session, _cs, _args| stacktrace_command(debugger, session),
            },
        )
    }
}


fn exit_command() -> Result<bool>
{
    Ok(true)
}


fn status_command(session: &mut probe_rs::Session) -> Result<bool>
{
    let mut core = session.core(0)?;
    let status = core.status()?;

    println!("Status: {:?}", &status);

    if status.is_halted() {
        let pc = core.read_core_reg(core.registers().program_counter())?;
        println!("Core halted at address {:#010x}", pc);
    }

    Ok(false)
}


fn print_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>,
                                            session: &mut probe_rs::Session,
                                            args:   &[&str]
                                            ) -> Result<bool>
{
    let mut core = session.core(0)?;
    let status = core.status()?;

    if status.is_halted() {
        let pc  = core.read_core_reg(core.registers().program_counter())?;
        let var = args[0];

        let unit = get_current_unit(&debugger.dwarf, pc)?;
        //println!("{:?}", unit.name.unwrap().to_string());
        
        let value = debugger.find_variable(&mut core, &unit, pc, var);
        
        match value {
            Ok(val)     => println!("{} = {}", var, val),
            Err(_err)   => println!("Could not find {}", var),
        };
    } else {
        println!("CPU must be halted to run this command");
        println!("Status: {:?}", &status);
    }

    Ok(false)
}


pub fn run_command(session: &mut probe_rs::Session, breakpoints: &Vec<u32>) -> Result<bool>
{
    let mut core = session.core(0)?;
    let status = core.status()?;

    if status.is_halted() {
        let _cpu_info = continue_fix(&mut core, breakpoints)?;
        core.run()?;    
    }

    info!("Core status: {:?}", core.status()?);

    Ok(false)
}


pub fn step_command(session: &mut probe_rs::Session, breakpoints: &Vec<u32>, print: bool) -> Result<bool>
{
    let mut core = session.core(0)?;
    let status = core.status()?;

    if status.is_halted() {
        let cpu_info = continue_fix(&mut core, breakpoints)?;
        info!("Stept to pc = 0x{:08x}", cpu_info.pc);

        if print {
            println!("Core stopped at address 0x{:08x}", cpu_info.pc);
        }
    }

    Ok(false)
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
                    if code[1] == 190 && code[0] == 0 { // bkpt == 0xbe00 for coretex-m
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


pub fn halt_command(session: &mut probe_rs::Session, cs: &mut capstone::Capstone, print: bool) -> Result<bool>
{
    let mut core = session.core(0)?;
    let status = core.status()?;

    if status.is_halted() {
        warn!("Core is already halted, status: {:?}", status);
        if print {
            println!("Core is already halted, status: {:?}", status);
        }
    } else {
        let cpu_info = core.halt(Duration::from_millis(100))?;
        info!("Core halted at pc = 0x{:08x}", cpu_info.pc);

        if print {
            let mut code = [0u8; 16 * 2];

            core.read_8(cpu_info.pc, &mut code)?;


            let insns = cs.disasm_all(&code, cpu_info.pc as u64)
                .expect("Failed to disassemble");
            
            for i in insns.iter() {
                let mut spacer = "  ";
                if i.address() == cpu_info.pc as u64 {
                    spacer = "> ";
                }
                println!("{}{}", spacer, i);
            }
        }
    }

    Ok(false)
}


fn regs_command(session: &mut probe_rs::Session) -> Result<bool>
{
    let mut core = session.core(0)?;
    let register_file = core.registers();

    for register in register_file.registers() {
        let value = core.read_core_reg(register)?;

        println!("{}:\t{:#010x}", register.name(), value)
    }

    Ok(false)
}


fn reset_command(session: &mut probe_rs::Session) -> Result<bool>
{
    let mut core = session.core(0)?;
    core.halt(Duration::from_millis(100))?;
    core.reset_and_halt(Duration::from_millis(100))?;

    Ok(false)
}


fn read_command(session: &mut probe_rs::Session,
                args:   &[&str]
                ) -> Result<bool>
{
    let mut core = session.core(0)?;
    let address = args[0].parse::<u32>()?;

    let mut buff = vec![0u32; 1];

    core.read_32(address, &mut buff)?;

    println!("{:#10x} = {:#10x}", address, buff[0]);

    Ok(false)
}


fn set_breakpoint_command<R: Reader<Offset = usize>>(debugger:  &mut Debugger<R>,
                                                     session:   &mut probe_rs::Session,
                                                     args:      &[&str],
                                                     print:     bool
                                                     ) -> Result<bool>
{
    let mut core = session.core(0)?;
    let address = args[0].parse::<u32>()?; 
    let num_bkpt = debugger.breakpoints.len() as u32;
    let tot_bkpt = core.get_available_breakpoint_units()?;

    if num_bkpt < tot_bkpt {
        core.set_hw_breakpoint(address)?;
        debugger.breakpoints.push(address);

        
        info!("Breakpoint set at: 0x{:08x}", address);
        if print {
            println!("Breakpoint set at: 0x{:08x}", address);
        }
    } else {
        if print {
            println!("All hw breakpoints are already set");
        }
    }

    Ok(false)
}


fn clear_breakpoint_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>,
                                                       session: &mut probe_rs::Session,
                                                       args:   &[&str],
                                                       print: bool
                                                       ) -> Result<bool>
{
    let mut core = session.core(0)?;
    if args.len() < 1 {
        if print {
            println!("Command requires an address(decimal number)");
        }
        return Ok(false);
    }

    let address = match args[0].parse::<u32>() {
        Ok(val)     => val,
        Err(err)    => {
            debug!("Failed to parse argument: {:?}", err);
            if print {
                println!("Command requires an address(decimal number)");
            }
            return Ok(false);
        },
    };

    for i in 0..debugger.breakpoints.len() {
        if address == debugger.breakpoints[i] {
            core.clear_hw_breakpoint(address)?;
            debugger.breakpoints.remove(i);

            info!("Breakpoint cleared from: 0x{:08x}", address);
            if print {
                println!("Breakpoint cleared from: 0x{:08x}", address);
            }
            break;
        }
    }

    Ok(false)
}


fn clear_all_breakpoints_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>,
                                                            session:    &mut probe_rs::Session,
                                                            print: bool
                                                            ) -> Result<bool>
{
    let mut core = session.core(0)?;
    for bkpt in &debugger.breakpoints {
        core.clear_hw_breakpoint(*bkpt)?;
    }
    debugger.breakpoints = vec!();

    info!("All breakpoints cleard");
    if print {
        println!("All breakpoints cleared");
    }

    Ok(false)
}


fn num_breakpoints_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>,
                                                      session:  &mut probe_rs::Session,
                                                     ) -> Result<bool>
{ 
    let mut core = session.core(0)?;
    println!("Number of hw breakpoints: {}/{}",
             debugger.breakpoints.len(),
             core.get_available_breakpoint_units()?);
    Ok(false)
}


fn code_command(session: &mut probe_rs::Session, cs: &mut capstone::Capstone) -> Result<bool>
{
    let mut core = session.core(0)?;
    let status = core.status()?;

    if status.is_halted() {
        let pc = core.registers().program_counter();
        let pc_val = core.read_core_reg(pc)?;

        let mut code = [0u8; 16 * 2];

        core.read_8(pc_val, &mut code)?;

        let insns = cs.disasm_all(&code, pc_val as u64)
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

    Ok(false)
}


fn stacktrace_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>,
                                                 session: &mut probe_rs::Session
                                                ) -> Result<bool>
{ 
    let mut core = session.core(0)?;
    println!("result: {:#?}", debugger.get_current_stacktrace(&mut core)?);
    Ok(false)
}


// Returns true if it has hit a brekpoint
pub fn hit_breakpoint(session: &mut probe_rs::Session) -> Result<bool>
{
    let mut core = session.core(0)?;
    let status = core.status()?;

    debug!("Status: {:?}", &status);

    match status {
        probe_rs::CoreStatus::Halted(r)  => {
            match r {
                probe_rs::HaltReason::Breakpoint => {
                    return Ok(true);
                },
                _ => return Ok(false),
            };
        },
        _ => return Ok(false),
    };
}

