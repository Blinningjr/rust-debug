use debugserver_types::Breakpoint;
use super::debug_request::DebugRequest;
use super::super::debugger::stacktrace::StackFrame;
use probe_rs::CoreStatus;


#[derive(Debug, Clone)]
pub enum DebugResponse {
    Attach,
    Status { status: CoreStatus, pc: Option<u32>},
    Exit,
    Continue,
    Step,
    Halt { message: Option<String> },
    SetBinary,
    Flash { message: Option<String> },
    Reset { message: Option<String> },
    Read { address: u32, value: Vec<u8> },
    StackTrace { stack_trace: Vec<StackFrame> },
    SetProbeNumber,
    SetChip,
    Variable { name: String, value: Option<String>, message: Option<String>},
    Registers { registers: Vec<(String, u32)> },
    SetBreakpoint,
    SetBreakpoints { breakpoints: Vec<Breakpoint> },
    ClearBreakpoint,
    ClearAllBreakpoints,
    Code { pc: u32, instructions: Vec<(u32, String)> },
    Stack { stack_pointer: u32, stack: Vec<u32> },
    Error { message: String, request: Option<DebugRequest> },
    SetCWD,
}

