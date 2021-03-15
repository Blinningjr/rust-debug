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


pub struct Command<R: Reader<Offset = usize>> {
    pub name:           &'static str,
    pub short:          &'static str,
    pub description:    &'static str,
    pub function:       fn(debugger: &mut Debugger<R>,
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
                function:       |_debugger, _args| exit_command(),
            },
            Command {
                name:           "status",
                short:          "s",
                description:    "Show current status of CPU",
                function:       |debugger, _args| status_command(&mut debugger.core),
            },
            Command {
                name:           "print",
                short:          "p",
                description:    "Evaluate variable",
                function:       |debugger, args| print_command(debugger, args),
            },
            Command {
                name:           "run",
                short:          "r",
                description:    "Resume execution of the CPU",
                function:       |debugger, _args| run_command(&mut debugger.core),
            },
            Command {
                name:           "step",
                short:          "sp",
                description:    "Step a single instruction",
                function:       |debugger, _args| step_command(&mut debugger.core),
            },
            Command {
                name:           "halt",
                short:          "h",
                description:    "Stop the CPU",
                function:       |debugger, _args| halt_command(&mut debugger.core),
            },
            Command {
                name:           "registers",
                short:          "regs",
                description:    "Show CPU register values",
                function:       |debugger, _args| regs_command(&mut debugger.core),
            },
            Command {
                name:           "reset",
                short:          "rt",
                description:    "Reset the CPU",
                function:       |debugger, _args| reset_command(&mut debugger.core),
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


fn run_command(core: &mut Core) -> Result<bool>
{
    let status = core.status()?;

    if status.is_halted() {
        let _cpu_info = continue_fix(core)?;
        core.run()?;
    }

    Ok(false)
}


fn step_command(core: &mut Core) -> Result<bool>
{
    let status = core.status()?;

    if status.is_halted() {
        let cpu_info = continue_fix(core)?;
        println!("Core stopped at address 0x{:08x}", cpu_info.pc);
    }

    Ok(false)
}

fn continue_fix(core: &mut Core) -> Result<CoreInformation, probe_rs::Error>
{
        let pc = core.registers().program_counter();
        let pc_val = core.read_core_reg(pc)?;

        // NOTE: Increment with 2 because ARM instuctions are usually 16-bits.
        let step_pc = pc_val + 0x2; // TODO: Fix for other CPU types.

        core.write_core_reg(pc.into(), step_pc)?;

        core.step()
}


fn halt_command(core: &mut Core) -> Result<bool>
{
    let cpu_info = core.halt(Duration::from_millis(100))?;
    println!("Core stopped at address 0x{:08x}", cpu_info.pc);

    let mut code = [0u8; 16 * 2];

    core.read_8(cpu_info.pc, &mut code)?;

    for (offset, instruction) in code.iter().enumerate() {
        println!(
            "{:#010x}:\t{:010x}",
            cpu_info.pc + offset as u32,
            instruction
        );
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

