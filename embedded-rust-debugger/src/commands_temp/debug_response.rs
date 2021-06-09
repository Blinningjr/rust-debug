use debugserver_types::Breakpoint;
use super::super::debugger::stacktrace::StackFrame;


#[derive(Debug, Clone)]
pub enum DebugResponse {
    Status,
    Exit,
    Continue,
    Step,
    Halt { pc: u32 },
    SetBinary,
    Flash { message: Option<String> },
    Reset { message: Option<String> },
    Read { address: u32, value: Vec<u8> },
    StackTrace { stack_trace: Vec<StackFrame> },
    SetProbeNumber,
    SetChip,
    Variable,
    Registers,
    SetBreakpoint,
    SetBreakpoints { breakpoints: Vec<Breakpoint> },
    ClearBreakpoint,
    ClearAllBreakpoints,
    Code,
    Stack,
}

