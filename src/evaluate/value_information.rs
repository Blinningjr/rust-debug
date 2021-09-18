#[derive(Debug, Clone)]
pub struct ValueInformation {
    pub raw: Option<Vec<u8>>, // byte size and raw value
    pub pieces: Vec<ValuePiece>,
}

impl ValueInformation {
    pub fn new(raw: Option<Vec<u8>>, pieces: Vec<ValuePiece>) -> ValueInformation {
        ValueInformation {
            raw: raw,
            pieces: pieces,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValuePiece {
    Register { register: u16, byte_size: usize },
    Memory { address: u32, byte_size: usize },
    Dwarf { value: Option<gimli::Value> },
}
