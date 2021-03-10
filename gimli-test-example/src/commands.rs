use super::{
    Core,
    Dwarf,
    Reader,
    debugger_cli::{
        DebuggerCli,
    },
    get_current_unit,
    Debugger,
};


pub struct Command<R> {
    pub name:           &'static str,
    pub description:    &'static str,
    pub function:       fn(core:    &mut Core,
                           dwarf:   &Dwarf<R>,
                           args:    &[&str]
                           ) -> Result<bool, &'static str>,
}

impl<R: Reader<Offset = usize>> Command<R> {
    pub fn init_commands() -> Vec<Command<R>> {
        vec!(
            Command {
                name:           "exit",
                description:    "Exit the debugger",
                function:       |core, dwarf, args| exit_command(),
            },
            Command {
                name:           "status",
                description:    "Show current status of CPU",
                function:       |core, _dwarf, _args| status_command(core),
            },
            Command {
                name:           "print",
                description:    "Evaluate variable",
                function:       |core, dwarf, args| print_command(core, dwarf, args),
            },
            Command {
                name:           "run",
                description:    "Resume execution of the CPU",
                function:       |core, _dwarf, _args| run_command(core),
            },
        )
    }
}


fn exit_command() -> Result<bool, &'static str>
{
    Ok(true)
}


fn status_command(core: &mut Core) -> Result<bool, &'static str>
{
    let status = core.status().unwrap();

    println!("Status: {:?}", &status);

    if status.is_halted() {
        let pc = core.read_core_reg(core.registers().program_counter()).unwrap();
        println!("Core halted at address {:#010x}", pc);
    }

    Ok(false)
}


fn print_command<R: Reader<Offset = usize>>(core:   &mut Core,
                                            dwarf:  &Dwarf<R>,
                                            args:   &[&str]
                                            ) -> Result<bool, &'static str>
{
    let status = core.status().unwrap();
    if status.is_halted() {
        let pc  = core.read_core_reg(core.registers().program_counter()).unwrap();
        let var = args[0];

        let unit = get_current_unit(&dwarf, pc).map_err(|_| "Can't find the current dwarf unit")?;
        println!("{:?}", unit.name.unwrap().to_string());

        //let mut debugger = Debugger::new(core, dwarf, &unit, pc);
        unimplemented!();
    }
    Ok(false)
}


fn run_command(core: &mut Core) -> Result<bool, &'static str>
{
    core.run().map_err(|_| "Failed to continue")?;
    Ok(false)
}

