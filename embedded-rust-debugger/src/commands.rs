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
                function:       |_debugger, _cs, _args| exit_command(),
            },
            Command {
                name:           "status",
                short:          "s",
                description:    "Show current status of CPU",
                function:       |debugger, _cs, _args| status_command(&mut debugger.core),
            },
            Command {
                name:           "print",
                short:          "p",
                description:    "Evaluate variable",
                function:       |debugger, _cs, args| print_command(debugger, args),
            },
            Command {
                name:           "run",
                short:          "r",
                description:    "Resume execution of the CPU",
                function:       |debugger, _cs, _args| run_command(&mut debugger.core, &debugger.breakpoints),
            },
            Command {
                name:           "step",
                short:          "sp",
                description:    "Step a single instruction",
                function:       |debugger, _cs, _args| step_command(&mut debugger.core, &debugger.breakpoints, true),
            },
            Command {
                name:           "halt",
                short:          "h",
                description:    "Stop the CPU",
                function:       |debugger, cs, _args| halt_command(&mut debugger.core, cs, true),
            },
            Command {
                name:           "registers",
                short:          "regs",
                description:    "Show CPU register values",
                function:       |debugger, _cs, _args| regs_command(&mut debugger.core),
            },
            Command {
                name:           "reset",
                short:          "rt",
                description:    "Reset the CPU",
                function:       |debugger, _cs, _args| reset_command(&mut debugger.core),
            },
            Command {
                name:           "read",
                short:          "rd",
                description:    "Read 32bit value from memory",
                function:       |debugger, _cs, args| read_command(&mut debugger.core, args),
            },
            Command {
                name:           "set_breakpoint",
                short:          "bkpt",
                description:    "Set breakpoint at an address",
                function:       |debugger, _cs, args| set_breakpoint_command(debugger, args, true),
            },
            Command {
                name:           "clear_breakpoint",
                short:          "cbkpt",
                description:    "Clear breakpoint from an address",
                function:       |debugger, _cs, args| clear_breakpoint_command(debugger, args, true),
            },
            Command {
                name:           "clear_all_breakpoints",
                short:          "cabkpt",
                description:    "Clear all breakpoints",
                function:       |debugger, _cs, _args| clear_all_breakpoints_command(debugger, true),
            },
            Command {
                name:           "num_breakpoints",
                short:          "nbkpt",
                description:    "Get total number of hw breakpoints",
                function:       |debugger, _cs, _args| num_breakpoints_command(debugger),
            },
            Command {
                name:           "code",
                short:          "ce",
                description:    "Print first 16 lines of assembly code",
                function:       |debugger, cs, _args| code_command(&mut debugger.core, cs),
            },
            Command {
                name:           "stacktrace",
                short:          "st",
                description:    "Print stack trace",
                function:       |debugger, _cs, _args| stacktrace_command(debugger),
            },
        )
    }
}


fn exit_command() -> Result<bool>
{
    Ok(true)
}


fn status_command(core: &mut Core) -> Result<bool>
{
    let status = core.status()?;

    println!("Status: {:?}", &status);

    if status.is_halted() {
        let pc = core.read_core_reg(core.registers().program_counter())?;
        println!("Core halted at address {:#010x}", pc);
    }

    Ok(false)
}


fn print_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>,
                                            args:   &[&str]
                                            ) -> Result<bool>
{
    let status = debugger.core.status()?;

    if status.is_halted() {
        let pc  = debugger.core.read_core_reg(debugger.core.registers().program_counter())?;
        let var = args[0];

        let unit = get_current_unit(&debugger.dwarf, pc)?;
        //println!("{:?}", unit.name.unwrap().to_string());
        
        let value = debugger.find_variable(&unit, pc, var);
        
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


pub fn run_command(core: &mut Core, breakpoints: &Vec<u32>) -> Result<bool>
{
    let status = core.status()?;

    if status.is_halted() {
        let _cpu_info = continue_fix(core, breakpoints)?;
        core.run()?;    
    }

    info!("Core status: {:?}", core.status()?);

    Ok(false)
}


pub fn step_command(core: &mut Core, breakpoints: &Vec<u32>, print: bool) -> Result<bool>
{
    let status = core.status()?;

    if status.is_halted() {
        let cpu_info = continue_fix(core, breakpoints)?;
        info!("Stept to pc = 0x{:08x}", cpu_info.pc);

        if print {
            println!("Core stopped at address 0x{:08x}", cpu_info.pc);
        }
    }

    Ok(false)
}

fn continue_fix(core: &mut Core, breakpoints: &Vec<u32>) -> Result<CoreInformation, probe_rs::Error>
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


pub fn halt_command(core: &mut Core, cs: &mut capstone::Capstone, print: bool) -> Result<bool>
{
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


fn regs_command(core: &mut Core) -> Result<bool>
{
    let register_file = core.registers();

    for register in register_file.registers() {
        let value = core.read_core_reg(register)?;

        println!("{}:\t{:#010x}", register.name(), value)
    }

    Ok(false)
}


fn reset_command(core: &mut Core) -> Result<bool>
{
    core.halt(Duration::from_millis(100))?;
    core.reset_and_halt(Duration::from_millis(100))?;

    Ok(false)
}


fn read_command(core: &mut Core,
                args:   &[&str]
                ) -> Result<bool>
{
    let address = args[0].parse::<u32>()?;

    let mut buff = vec![0u32; 1];

    core.read_32(address, &mut buff)?;

    println!("{:#10x} = {:#10x}", address, buff[0]);

    Ok(false)
}


fn set_breakpoint_command<R: Reader<Offset = usize>>(debugger:  &mut Debugger<R>,
                                                     args:      &[&str],
                                                     print:     bool
                                                     ) -> Result<bool>
{
    let address = args[0].parse::<u32>()?; 
    let num_bkpt = debugger.breakpoints.len() as u32;
    let tot_bkpt = debugger.core.get_available_breakpoint_units()?;

    if num_bkpt < tot_bkpt {
        debugger.core.set_hw_breakpoint(address)?;
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
                                                       args:   &[&str],
                                                       print: bool
                                                       ) -> Result<bool>
{
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
            debugger.core.clear_hw_breakpoint(address)?;
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


fn clear_all_breakpoints_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>, print: bool) -> Result<bool>
{
    for bkpt in &debugger.breakpoints {
        debugger.core.clear_hw_breakpoint(*bkpt)?;
    }
    debugger.breakpoints = vec!();

    info!("All breakpoints cleard");
    if print {
        println!("All breakpoints cleared");
    }

    Ok(false)
}


fn num_breakpoints_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>) -> Result<bool>
{ 
    println!("Number of hw breakpoints: {}/{}",
             debugger.breakpoints.len(),
             debugger.core.get_available_breakpoint_units()?);
    Ok(false)
}


fn code_command(core: &mut Core, cs: &mut capstone::Capstone) -> Result<bool>
{
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


fn stacktrace_command<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>) -> Result<bool>
{ 
    println!("result: {:#?}", debugger.get_current_stacktrace()?);
    Ok(false)
}


// Returns true if it has hit a brekpoint
pub fn hit_breakpoint(core: &mut Core) -> Result<bool>
{
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

