use std::collections::HashMap;
use std::convert::TryInto;


#[derive(Debug, Clone)]
pub struct MemoryAndRegisters {
    pub memory: HashMap<u32, u8>,
    pub registers: HashMap<u16, u32>,
    stashed_registers: Option<HashMap<u16, u32>>,
}

impl MemoryAndRegisters {
    pub fn new() -> MemoryAndRegisters {
        MemoryAndRegisters {
            memory: HashMap::new(),
            registers: HashMap::new(),
            stashed_registers: None,
        }
    }

    pub fn add_to_memory(&mut self, address: u32, value: Vec<u8>) {
        for i in 0..value.len() {
            self.memory.insert(address + i as u32, value[i]);
        }
    }


    pub fn add_to_registers(&mut self, register: u16, value: u32) {
        self.registers.insert(register, value);
    }


    pub fn get_address_value(&self, address: &u32) -> Option<u32> {
        let mut result = vec!();
        for i in 0..4 {
            match self.memory.get(&(*address + i as u32)) {
                Some(val) => result.push(*val),
                None => return None,
            };
        }

        Some(u32::from_le_bytes(result.as_slice().try_into().unwrap()))
    }


    pub fn get_addresses(&self, address: &u32, num_bytes: usize) -> Option<Vec<u8>> {
        let mut result = vec!();
        for i in 0..num_bytes {
            match self.memory.get(&(*address + i as u32)) {
                Some(val) => result.push(*val),
                None => return None,
            };
        }

        Some(result)
    }
    

    pub fn get_register_value(&self, register: &u16) -> Option<&u32> {
        self.registers.get(register)
    }


    pub fn clear(&mut self) {
        self.memory = HashMap::new();
        self.registers = HashMap::new();
        self.stashed_registers = None;
    }


    pub fn stash_registers(&mut self) {
        self.stashed_registers = Some(self.registers.clone());
        self.registers = HashMap::new();
    }

    
    pub fn pop_stashed_registers(&mut self) {
        if let Some(registers) = self.stashed_registers.clone() {
            self.registers = registers;
        }
        self.stashed_registers = None;
    }
}

