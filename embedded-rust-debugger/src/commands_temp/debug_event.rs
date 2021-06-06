use probe_rs::HaltReason;


#[derive(Debug, Clone)]
pub enum DebugEvent {
    Halted { pc: u32, reason: HaltReason },
}

