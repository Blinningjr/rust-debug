use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Registers {
    pub registers: HashMap<u16, u32>,
    stashed_registers: Option<HashMap<u16, u32>>,

    pub program_counter_register: Option<usize>,
    pub link_register: Option<usize>,
    pub stack_pointer_register: Option<usize>,

    pub cfa: Option<u32>, // Canonical Frame Address
}

impl Registers {
    pub fn new() -> Registers {
        Registers {
            registers: HashMap::new(),
            stashed_registers: None,
            program_counter_register: None,
            link_register: None,
            stack_pointer_register: None,
            cfa: None,
        }
    }

    pub fn add_register_value(&mut self, register: u16, value: u32) {
        self.registers.insert(register, value);
    }

    pub fn get_register_value(&self, register: &u16) -> Option<&u32> {
        self.registers.get(register)
    }

    pub fn clear(&mut self) {
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
