use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// A struct to hold the register values and other register information.
#[derive(Debug, Clone)]
pub struct Registers {
    /// Holds all the register values.
    pub registers: HashMap<u16, u32>,

    /// Holds stashed register values. It is used when evaluating values lower down in the stack.
    stashed_registers: Option<HashMap<u16, u32>>,

    /// The register number which is the program counter register.
    pub program_counter_register: Option<usize>,

    /// The register number which is the link register.
    pub link_register: Option<usize>,

    /// The register number which is the stack pointer register.
    pub stack_pointer_register: Option<usize>,

    /// Canonical Frame Address, which is sometimes needed to evaluate variables.
    pub cfa: Option<u32>, // Canonical Frame Address
}

impl Default for Registers {
    /// Creates a empty `Registers` struct.
    fn default() -> Registers {
        Registers {
            registers: HashMap::new(),
            stashed_registers: None,
            program_counter_register: None,
            link_register: None,
            stack_pointer_register: None,
            cfa: None,
        }
    }
}
impl Registers {
    /// Add a register value to the struct.
    ///
    /// Description:
    ///
    /// * `register` - The register to add a value for.
    /// * `value` - The value that will be stored for that registry.
    ///
    /// This function will add the `value` to the `self.registers` HashMap with `register` as the hash
    /// number.
    pub fn add_register_value(&mut self, register: u16, value: u32) {
        self.registers.insert(register, value);
    }

    /// Retrieve a register value.
    ///
    /// Description:
    ///
    /// * `register` - The register to get the value from.
    ///
    /// Will retrieve the `register` value from the `self.registers` HashMap.
    pub fn get_register_value(&self, register: &u16) -> Option<&u32> {
        self.registers.get(register)
    }

    /// Sets all the register values to `None` in the struct.
    pub fn clear(&mut self) {
        self.registers = HashMap::new();
        self.stashed_registers = None;
    }

    /// Temporally stash the current register values.
    ///
    /// Description:
    ///
    /// The current register values will be stashed allowing for other register values to be used
    /// in the evaluation of variables.
    /// This is only used when evaluating `StackFrame`s because preserved register values need to
    /// be used in the evaluation.
    pub fn stash_registers(&mut self) {
        self.stashed_registers = Some(self.registers.clone());
        self.registers = HashMap::new();
    }

    /// Pop the stashed register values.
    ///
    /// Description:
    ///
    /// This is used to pop back the stashed registers into `self.registers`.
    /// This function is only called after doing a stack trace because preserved register values is
    /// used for evaluating the `StackFrame`s.
    pub fn pop_stashed_registers(&mut self) {
        if let Some(registers) = self.stashed_registers.clone() {
            self.registers = registers;
        }
        self.stashed_registers = None;
    }

    /// Get registers as a Vec of `Variables`
    ///
    /// Description:
    ///
    /// This is used to get the register as a Vec of Variables.
    pub fn get_registers_as_list(&self) -> Vec<(u16, u32)> {
        let mut res: Vec<(u16, u32)> = self
            .registers
            .clone()
            .into_iter()
            .map(|(id, score)| (id, score))
            .collect();
        res.sort_by(|a, b| b.0.cmp(&a.0));
        res
    }

    /// Get the `pc` register value.
    ///
    /// Description:
    ///
    /// This is the same as calling `get_register_value` with the pc register a as input.
    pub fn get_pc_register(&self) -> Result<Option<&u32>> {
        let pc_reg = self
            .program_counter_register
            .ok_or_else(|| anyhow!("Requires that the program counter register is known"))?;
        Ok(self.get_register_value(&(pc_reg as u16)))
    }
}
