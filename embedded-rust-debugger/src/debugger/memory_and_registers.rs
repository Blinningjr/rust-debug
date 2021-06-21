use std::collections::HashMap;


#[derive(Debug, Clone)]
pub struct MemoryAndRegisters {
    pub memory: HashMap<u32, u32>,
    pub registers: HashMap<u16, u32>,
}

impl MemoryAndRegisters {
    pub fn new() -> MemoryAndRegisters {
        MemoryAndRegisters {
            memory: HashMap::new(),
            registers: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.memory = HashMap::new();
        self.registers = HashMap::new();
    }


    pub fn add_to_memory(&mut self, address: u32, value: u32) {
        self.memory.insert(address, value);
    }


    pub fn add_to_registers(&mut self, register: u16, value: u32) {
        self.registers.insert(register, value);
    }


    pub fn get_address_value(&self, address: &u32) -> Option<&u32> {
        self.memory.get(address)
    }
    

    pub fn get_register_value(&self, register: &u16) -> Option<&u32> {
        self.registers.get(register)
    }
}

