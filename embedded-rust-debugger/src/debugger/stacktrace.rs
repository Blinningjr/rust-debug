/**
 * Good gimli sources:
 * https://docs.rs/gimli/0.23.0/gimli/read/struct.DebugFrame.html
 * https://docs.rs/gimli/0.23.0/gimli/read/trait.UnwindSection.html
 *
 * Dwarf source: Dwarf 5 section 6.4.1
 */


use super::{
    Debugger,
};

use gimli::{
    Reader,
    DebugFrame,
    UnwindSection,
    RegisterRule::*,
};

use anyhow::{
    anyhow,
    Result,
};

use probe_rs::MemoryInterface;

use log::{
    debug,
};


#[derive(Debug, Clone)]
pub struct StackFrame {
    pub call_frame: CallFrame,
    pub name: String,
    pub source: SourceReference,
    pub variables: Vec<(Option<String>, String)>,
}


#[derive(Debug, Clone)]
pub struct SourceReference {
    pub directory:  Option<String>,
    pub file:       Option<String>,
    pub line:       Option<u64>,
    pub column:     Option<u64>,
}


#[derive(Debug, Clone)]
pub struct CallFrame {
    pub id:             u64,
    pub registers:      [Option<u32>; 16],
    pub code_location:  u64,
    pub cfa:            Option<u32>,
    pub start_address:  u64,
    pub end_address:    u64,
}


pub fn create_call_stacktrace<R: Reader<Offset = usize>>(debugger: &mut Debugger<R>, core: &mut probe_rs::Core) -> Result<Vec<CallFrame>> {
    let mut cfi = CallFrameIterator::new(debugger, core)?;
    let mut stacktrace = vec!();

    loop {
        match cfi.next(core)? {
            Some(val)   => {
                stacktrace.push(val);
            },
            None        => {
                //stacktrace = stacktrace.iter().rev().cloned().collect();
                return Ok(stacktrace);
            },

        };
    }
}


pub struct CallFrameIterator<'a, 'b, R: Reader<Offset = usize>> {
    debugger:       &'b mut Debugger<'a, R>,
    frame_counter:  u64,
    code_location:  Option<u64>,
    registers:      [Option<u32>; 16],
        
    // Optionally provide base addresses for any relative pointers. If a
    // base address isn't provided and a pointer is found that is relative to
    // it, we will return an `Err`.
    bases:  gimli::BaseAddresses,

    // This context is reusable, which cuts down on heap allocations.
    ctx:    gimli::UninitializedUnwindContext<R>,
}


impl<'a, 'b, R: Reader<Offset = usize>> CallFrameIterator<'a, 'b, R> {
    pub fn new(debugger: &'b mut Debugger<'a, R>, core: &mut probe_rs::Core) -> Result<CallFrameIterator<'a, 'b, R>>
    {
        let pc =        core.registers().program_counter();
        let pc_val =    core.read_core_reg(pc)?;

        let mut register = [None; 16];
        for i in 0..16 {
            register[i as usize] = core.read_core_reg(i).ok();
        }

        Ok(CallFrameIterator {
            debugger:       debugger,
            frame_counter:  0,
            code_location:  Some(pc_val as u64),
            registers:      register, 
            bases:          gimli::BaseAddresses::default(),
            ctx:            gimli::UninitializedUnwindContext::new(),
        })
    }

    pub fn next(&mut self,
                core: &mut probe_rs::Core
                ) -> Result<Option<CallFrame>>
    {
        let pc_reg: u16 =   probe_rs::CoreRegisterAddress::from(core.registers().program_counter()).0;
        let link_reg: u16 = probe_rs::CoreRegisterAddress::from(core.registers().return_address()).0;
        let sp_reg: u16 =   probe_rs::CoreRegisterAddress::from(core.registers().stack_pointer()).0;

        let code_location = match self.code_location {
            Some(val)   => val,
            None        => return Ok(None),
        };

        let unwind_info = match self.debugger.debug_frame.unwind_info_for_address(
            &self.bases,
            &mut self.ctx,
            code_location,
            gimli::DebugFrame::cie_from_offset,
        ) {
            Ok(val) => val,
            Err(_)  => return Ok(None),
        };

        let cfa = self.eval_cfa(&unwind_info)?;

        let mut registers = [None; 16];
        for i in 0..16 {
            let reg_rule = unwind_info.register(gimli::Register(i));

            registers[i as usize] = match reg_rule {
                Undefined => {
                    // Gimli doesn't allow us to distinguish if a rule is not
                    // present or actually set to Undefined in the call frame
                    // information.
                    // Source: https://github.com/probe-rs/probe-rs/blob/8112c28912125a54aad016b4b935abf168812698/probe-rs/src/debug/mod.rs#L254-L257
                    if i == sp_reg {
                        cfa
                    } else if i == link_reg {
                       self.registers[i as usize] 
                    } else if i == pc_reg {
                        Some(code_location as u32)
                    } else {
                        self.registers[i as usize] 
                        //None
                    }
                },
                SameValue => self.registers[i as usize],
                Offset(offset) => {
                    let address = (offset + match cfa {
                        Some(val) => i64::from(val),
                        None => return Err(anyhow!("Expected CFA to have a value")),
                    }) as u32;

                    let mut buff = vec![0u32; 1];

                    core.read_32(address, &mut buff)?;

                    Some(buff[0])
                },
                ValOffset(offset) => {
                    let value = (offset + match cfa {
                        Some(val)   => i64::from(val),
                        None        => return Err(anyhow!("Expected CFA to have a value")),
                    }) as u32;

                    Some(value)
                },
                Register(reg)       => self.registers[reg.0 as usize],
                Expression(expr)    => unimplemented!(), // TODO
                ValExpression(expr) => unimplemented!(), // TODO
                Architectural       => unimplemented!(), // TODO
            };
        }

        
        let cf = CallFrame {
            id:             self.frame_counter,
            registers:      registers,
            code_location:  code_location,
            cfa:            cfa,
            start_address:  unwind_info.start_address(),
            end_address:    unwind_info.end_address(),
        };

        self.registers = registers;

        self.frame_counter += 1;

        // Source: https://github.com/probe-rs/probe-rs/blob/8112c28912125a54aad016b4b935abf168812698/probe-rs/src/debug/mod.rs#L297-L302
        // Next function is where our current return register is pointing to.
        // We just have to remove the lowest bit (indicator for Thumb mode).
        //
        // We also have to subtract one, as we want the calling instruction for
        // a backtrace, not the next instruction to be executed.
        self.code_location = self.registers[link_reg as usize].map(|pc| u64::from(pc & !1) - 1);
        
        return Ok(Some(cf));
    }


    fn eval_cfa(&mut self, unwind_info: &gimli::UnwindTableRow<R>) -> Result<Option<u32>> {
        match unwind_info.cfa() {
            gimli::CfaRule::RegisterAndOffset   {register, offset}  => {
                let reg_val = match self.registers[register.0 as usize] {
                    Some(val)   => val,
                    None        => return Ok(None),
                };
                Ok(Some((i64::from(reg_val) + offset) as u32))
            },
            gimli::CfaRule::Expression          (expr)              => {
                unimplemented!(); // TODO
//                let mut eval = expr.evaluation(cie.encoding());
//                let mut result = eval.evaluate().unwrap();
            }, 
        }
    }
}

