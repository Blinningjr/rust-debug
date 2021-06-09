use std::path::PathBuf;

use debugserver_types::SourceBreakpoint;

#[derive(Debug, Clone)]
pub enum DebugRequest {
    Status,
    Exit,
    Continue,
    Step,
    Halt,
    SetBinary { path: PathBuf },
    Flash { reset_and_halt: bool },
    Reset { reset_and_halt: bool }, 
    Read { address: u32, byte_size: usize },
    StackTrace,
    SetProbeNumber { number: usize },
    SetChip { chip: String },
    Variable { name: String },
    Registers,
    SetBreakpoint { address: u32, source_file: Option<String>},
    SetBreakpoints { source_file: String, source_breakpoints: Vec<SourceBreakpoint> },
    ClearBreakpoint { address: u32 },
    ClearAllBreakpoints,
    Code,
    Stack,
}


