#[derive(Debug, Clone)]
pub enum DebugResponse {
    Exit,
    Continue,
    Step { pc: u32 },
    Halt { pc: u32 },
    SetBinary,
}

