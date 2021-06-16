/**
 * Good gimli sources:
 * https://docs.rs/gimli/0.23.0/gimli/read/struct.DebugFrame.html
 * https://docs.rs/gimli/0.23.0/gimli/read/trait.UnwindSection.html
 *
 * Dwarf source: Dwarf 5 section 6.4.1
 */

use gimli::DebugFrame;

use std::collections::HashMap;

use gimli::{
    Reader,
    UnwindSection,
    RegisterRule::*,
};

use anyhow::{
    anyhow,
    Result,
};

use log::trace;


#[derive(Debug, Clone)]
pub struct CallFrame {
    pub id:             u64,
    pub registers:      [Option<u32>; 16],
    pub code_location:  u64,
    pub cfa:            Option<u32>,
    pub start_address:  u64,
    pub end_address:    u64,
}


pub enum UnwindResult {
    Complete,
    RequiresAddress { address: u32 },
}


pub struct CallStackUnwinder<R: Reader<Offset = usize>> {
    program_counter_register:   usize,
    link_register:              usize,
    stack_pointer_register:     usize,

    code_location:  Option<u64>,
    registers:      [Option<u32>; 16],

    // Optionally provide base addresses for any relative pointers. If a
    // base address isn't provided and a pointer is found that is relative to
    // it, we will return an `Err`.
    bases:  gimli::BaseAddresses,

    // This context is reusable, which cuts down on heap allocations.
    ctx:    gimli::UninitializedUnwindContext<R>,

    call_stack: Vec<CallFrame>,

    memory: HashMap<u32, u32>,
}


impl<R: Reader<Offset = usize>> CallStackUnwinder<R> {
    pub fn new(program_counter_register:    usize,
               link_register:               usize,
               stack_pointer_register:      usize,
               registers:                   [u32;16]
               ) -> CallStackUnwinder<R>
    {
        let mut regs = [None;16];
        for i in 0..16 {
            regs[i] = Some(registers[i]);
        }
        CallStackUnwinder {
            program_counter_register:   program_counter_register,
            link_register:              link_register,
            stack_pointer_register:     stack_pointer_register,

            code_location:  Some(registers[program_counter_register] as u64),
            registers:      regs,

            bases:          gimli::BaseAddresses::default(),
            ctx:            gimli::UninitializedUnwindContext::new(),

            call_stack:     vec!(),

            memory: HashMap::new(),
        }
    }


    pub fn add_address(&mut self, address: u32, value: u32) {
        self.memory.insert(address, value);
    }


    pub fn get_call_stack(&self) -> Vec<CallFrame> {
        self.call_stack.clone()
    }


    pub fn unwind<'a>(&mut self,
                debug_frame: &'a DebugFrame<R>,
                ) -> Result<UnwindResult>
    {
        let code_location = match self.code_location {
            Some(val)   => val,
            None        => {
                trace!("Stoped unwinding call stack, because: Reached end of stack");
                return Ok(UnwindResult::Complete);
            },
        };


        let unwind_info = match debug_frame.unwind_info_for_address(
            &self.bases,
            &mut self.ctx,
            code_location,
            gimli::DebugFrame::cie_from_offset,
        ) {
            Ok(val) => val,
            Err(err)  => {
                trace!("Stoped unwinding call stack, because: {:?}", err);
                return Ok(UnwindResult::Complete);
            },
        };


        let cfa = self.unwind_cfa(&unwind_info)?;

        let mut registers = [None; 16];
        for i in 0..16 as usize {
            let reg_rule = unwind_info.register(gimli::Register(i as u16));

            registers[i] = match reg_rule {
                Undefined => {
                    // If the column is empty then it defaults to undefined.
                    // Source: https://github.com/gimli-rs/gimli/blob/00f4ee6a288d2e7f02b6841a5949d839e99d8359/src/read/cfi.rs#L2289-L2311
                    if i == self.stack_pointer_register {
                        cfa
                    } else if i == self.program_counter_register {
                        Some(code_location as u32)
                    } else {
                        None
                    }
                },
                SameValue => self.registers[i],
                Offset(offset) => {
                    println!("reg: {:?}, offset: {:?}", i, offset);
                    let address = (offset + match cfa {
                        Some(val) => i64::from(val),
                        None => return Err(anyhow!("Expected CFA to have a value")),
                    }) as u32;

                    let value = match self.memory.get(&address) {
                        Some(val) => *val,
                        None => return Ok(UnwindResult::RequiresAddress {
                            address: address,
                        }),
                    };

                    Some(value)
                },
                ValOffset(offset) => {
                    let value = (offset + match cfa {
                        Some(val)   => i64::from(val),
                        None        => return Err(anyhow!("Expected CFA to have a value")),
                    }) as u32;

                    Some(value)
                },
                Register(reg)       => self.registers[reg.0 as usize],
                Expression(_expr)    => unimplemented!(), // TODO
                ValExpression(_expr) => unimplemented!(), // TODO
                Architectural       => unimplemented!(), // TODO
            };
        }

        
        self.call_stack.push(CallFrame {
            id:             code_location,
            registers:      self.registers,
            code_location:  code_location,
            cfa:            cfa,
            start_address:  unwind_info.start_address(),
            end_address:    unwind_info.end_address(),
        });

        self.registers = registers;

        // Source: https://github.com/probe-rs/probe-rs/blob/8112c28912125a54aad016b4b935abf168812698/probe-rs/src/debug/mod.rs#L297-L302
        // Next function is where our current return register is pointing to.
        // We just have to remove the lowest bit (indicator for Thumb mode).
        //
        // We also have to subtract one, as we want the calling instruction for
        // a backtrace, not the next instruction to be executed.
        self.code_location = self.registers[self.link_register as usize].map(|pc| u64::from(pc & !1) - 1);
        
        self.unwind(debug_frame)
    }


    fn unwind_cfa(&mut self, unwind_info: &gimli::UnwindTableRow<R>) -> Result<Option<u32>> {
        match unwind_info.cfa() {
            gimli::CfaRule::RegisterAndOffset {register, offset} => {
                let reg_val = match self.registers[register.0 as usize] {
                    Some(val)   => val,
                    None        => return Ok(None),
                };
                Ok(Some((i64::from(reg_val) + offset) as u32))
            },
            gimli::CfaRule::Expression(_expr) => {
                unimplemented!(); // TODO
            }, 
        }
    }
}


