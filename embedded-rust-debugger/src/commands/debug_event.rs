use probe_rs::HaltReason;


//use debugserver_types::{
//    SourceBreakpoint,
//};


#[derive(Debug, Clone)]
pub enum DebugEvent {
    Halted { pc: u32, reason: HaltReason, hit_breakpoint_ids: Option<Vec<u32>> },
}


//#[derive(Debug, Clone)]
//pub struct BreakpointInfo {
//    id: u32,
//    verified: bool,
//    info: SourceBreakpoint,
//    address: Option<u64>,
//    location: Option<u32>,
//}

