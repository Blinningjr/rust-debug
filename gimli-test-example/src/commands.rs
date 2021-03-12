use super::{
    Core,
    Reader,
    get_current_unit,
    Debugger,
};


use anyhow::{
    Result,
};


pub struct Command<R: Reader<Offset = usize>> {
    pub name:           &'static str,
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
                description:    "Exit the debugger",
                function:       |_debugger, _args| exit_command(),
            },
            Command {
                name:           "status",
                description:    "Show current status of CPU",
                function:       |debugger, _args| status_command(&mut debugger.core),
            },
            Command {
                name:           "print",
                description:    "Evaluate variable",
                function:       |debugger, args| print_command(debugger, args),
            },
            Command {
                name:           "run",
                description:    "Resume execution of the CPU",
                function:       |debugger, _args| run_command(&mut debugger.core),
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
        println!("{} = {:#?}", var, value);
    }
    Ok(false)
}


fn run_command(core: &mut Core) -> Result<bool>
{
    core.run()?;
    Ok(false)
}

