#[derive(Debug, Clone)]
pub enum DebugResponse {
    Status,
    Exit,
    Continue,
    Step { pc: Option<u32> },
    Halt { pc: u32 },
    SetBinary,
    Flash { message: Option<String> },
    Reset { message: Option<String> },
    Read { address: u32, value: Vec<u8> },
    StackTrace,
    SetProbeNumber,
    SetChip,
    Variable,
    Registers,
    SetBreakpoint,
    ClearBreakpoint,
    ClearAllBreakpoints,
    Code,
    Stack,
}

