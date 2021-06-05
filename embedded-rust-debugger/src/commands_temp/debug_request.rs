use std::path::PathBuf;


#[derive(Debug, Clone)]
pub enum DebugRequest {
    Exit,
    Continue,
    Step,
    Halt,
    SetBinary { path: PathBuf },
}

