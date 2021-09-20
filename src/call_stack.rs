use crate::evaluate::evaluate::BaseValue;
/**
 * Good gimli sources:
 * https://docs.rs/gimli/0.23.0/gimli/read/struct.DebugFrame.html
 * https://docs.rs/gimli/0.23.0/gimli/read/trait.UnwindSection.html
 *
 * Dwarf source: Dwarf 5 section 6.4.1
 */
use crate::evaluate::{evaluate, EvaluatorValue};
use crate::registers::Registers;
use crate::source_information::SourceInformation;
use crate::utils::{die_in_range, get_current_unit};
use crate::variable::{is_variable_die, Variable};
use anyhow::{anyhow, Result};
use gimli::DebugFrame;
use gimli::{RegisterRule::*, UnwindSection};
use log::trace;
use std::convert::TryInto;

use gimli::{
    AttributeValue::DebugStrRef, DebuggingInformationEntry, Dwarf, EntriesTreeNode, Reader, Unit,
    UnitOffset, UnitSectionOffset,
};

pub trait MemoryAccess {
    fn get_address(&mut self, address: &u32, num_bytes: usize) -> Option<Vec<u8>>;

    fn get_register(&mut self, register: &u16) -> Option<u32>;
}

pub fn stack_trace<'a, R: Reader<Offset = usize>, M: MemoryAccess>(
    dwarf: &Dwarf<R>,
    debug_frame: &'a DebugFrame<R>,
    registers: Registers,
    memory: &mut M,
    cwd: &str,
) -> Result<Vec<StackFrame>> {
    let call_stacktrace = unwind_call_stack(registers.clone(), memory, debug_frame)?;

    let mut stack_trace = vec![];
    for call_frame in call_stacktrace {
        let stack_frame = create_stack_frame(dwarf, call_frame, &registers, memory, cwd)?;

        stack_trace.push(stack_frame);
    }
    Ok(stack_trace)
}

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub id: u64,
    pub registers: [Option<u32>; 16],
    pub code_location: u64,
    pub cfa: Option<u32>,
    pub start_address: u64,
    pub end_address: u64,
}

/**
 *  A function for retrieving the call stack
 */
pub fn unwind_call_stack<'a, R: Reader<Offset = usize>, M: MemoryAccess>(
    registers: Registers,
    memory: &mut M,
    debug_frame: &'a DebugFrame<R>,
) -> Result<Vec<CallFrame>> {
    let pc_reg = registers
        .program_counter_register
        .ok_or(anyhow!("Requires pc register id"))?;
    let link_reg = registers
        .link_register
        .ok_or(anyhow!("Requires pc register id"))?;
    let sp_reg = registers
        .stack_pointer_register
        .ok_or(anyhow!("Requires pc register id"))?;

    let mut regs = [None; 16];
    for (reg, val) in &registers.registers {
        regs[*reg as usize] = Some(*val);
    }
    let code_location = registers
        .get_register_value(&(pc_reg as u16))
        .map(|v| *v as u64);

    unwind_call_stack_recursive(
        debug_frame,
        memory,
        pc_reg,
        link_reg,
        sp_reg,
        code_location,
        regs,
        &mut gimli::BaseAddresses::default(),
        &mut gimli::UninitializedUnwindContext::new(),
    )
}

fn unwind_call_stack_recursive<'a, M: MemoryAccess, R: Reader<Offset = usize>>(
    debug_frame: &'a DebugFrame<R>,
    memory: &mut M,
    pc_reg: usize,
    link_reg: usize,
    sp_reg: usize,
    code_location: Option<u64>,
    mut unwind_registers: [Option<u32>; 16],
    base: &mut gimli::BaseAddresses,
    ctx: &mut gimli::UninitializedUnwindContext<R>,
) -> Result<Vec<CallFrame>> {
    let current_location = match code_location {
        Some(val) => val,
        None => {
            trace!("Stopped unwinding call stack, because: Reached end of stack");
            return Ok(vec![]);
        }
    };

    let unwind_info = match debug_frame.unwind_info_for_address(
        &base,
        ctx,
        current_location,
        gimli::DebugFrame::cie_from_offset,
    ) {
        Ok(val) => val,
        Err(err) => {
            trace!("Stopped unwinding call stack, because: {:?}", err);
            return Ok(vec![]);
        }
    };

    let cfa = unwind_cfa(unwind_registers, &unwind_info)?;

    let mut new_registers = [None; 16];
    for i in 0..16 as usize {
        let reg_rule = unwind_info.register(gimli::Register(i as u16));

        new_registers[i] = match reg_rule {
            Undefined => {
                // If the column is empty then it defaults to undefined.
                // Source: https://github.com/gimli-rs/gimli/blob/00f4ee6a288d2e7f02b6841a5949d839e99d8359/src/read/cfi.rs#L2289-L2311
                if i == sp_reg {
                    cfa
                } else if i == pc_reg {
                    Some(current_location as u32)
                } else {
                    None
                }
            }
            SameValue => unwind_registers[i],
            Offset(offset) => {
                let address = (offset
                    + match cfa {
                        Some(val) => i64::from(val),
                        None => return Err(anyhow!("Expected CFA to have a value")),
                    }) as u32;

                let value = {
                    let value = match memory.get_address(&address, 4) {
                        Some(val) => {
                            let mut result = vec![];
                            for v in val {
                                result.push(v);
                            }

                            u32::from_le_bytes(result.as_slice().try_into().unwrap())
                        }
                        None => panic!("tait not working"),
                    };
                    value
                };

                Some(value)
            }
            ValOffset(offset) => {
                let value = (offset
                    + match cfa {
                        Some(val) => i64::from(val),
                        None => return Err(anyhow!("Expected CFA to have a value")),
                    }) as u32;

                Some(value)
            }
            Register(reg) => unwind_registers[reg.0 as usize],
            Expression(_expr) => unimplemented!(),    // TODO
            ValExpression(_expr) => unimplemented!(), // TODO
            Architectural => unimplemented!(),        // TODO
        };
    }

    let mut call_stack = vec![CallFrame {
        id: current_location,
        registers: unwind_registers,
        code_location: current_location,
        cfa,
        start_address: unwind_info.start_address(),
        end_address: unwind_info.end_address(),
    }];

    unwind_registers = new_registers;

    // Source: https://github.com/probe-rs/probe-rs/blob/8112c28912125a54aad016b4b935abf168812698/probe-rs/src/debug/mod.rs#L297-L302
    // Next function is where our current return register is pointing to.
    // We just have to remove the lowest bit (indicator for Thumb mode).
    //
    // We also have to subtract one, as we want the calling instruction for
    // a backtrace, not the next instruction to be executed.
    let next_code_location = unwind_registers[link_reg as usize].map(|pc| u64::from(pc & !1) - 1);

    call_stack.append(&mut unwind_call_stack_recursive(
        debug_frame,
        memory,
        pc_reg,
        link_reg,
        sp_reg,
        next_code_location,
        unwind_registers,
        base,
        ctx,
    )?);
    return Ok(call_stack);
}

fn unwind_cfa<R: Reader<Offset = usize>>(
    registers: [Option<u32>; 16],
    unwind_info: &gimli::UnwindTableRow<R>,
) -> Result<Option<u32>> {
    match unwind_info.cfa() {
        gimli::CfaRule::RegisterAndOffset { register, offset } => {
            let reg_val = match registers[register.0 as usize] {
                Some(val) => val,
                None => return Ok(None),
            };
            Ok(Some((i64::from(reg_val) + offset) as u32))
        }
        gimli::CfaRule::Expression(_expr) => {
            unimplemented!(); // TODO
        }
    }
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub call_frame: CallFrame,
    pub name: String,
    pub source: SourceInformation,
    pub variables: Vec<Variable>,
}

impl StackFrame {
    pub fn find_variable(&self, name: &str) -> Option<&Variable> {
        for v in &self.variables {
            match &v.name {
                Some(var_name) => {
                    if var_name == name {
                        return Some(v);
                    }
                }
                None => (),
            };
        }

        return None;
    }
}

pub fn create_stack_frame<M: MemoryAccess, R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    call_frame: CallFrame,
    registers: &Registers,
    mem: &mut M,
    cwd: &str,
) -> Result<StackFrame> {
    // Find the corresponding function to the call frame.
    let (section_offset, unit_offset) = find_function_die(dwarf, call_frame.code_location as u32)?;
    let header =
        dwarf
            .debug_info
            .header_from_offset(match section_offset.as_debug_info_offset() {
                Some(val) => val,
                None => {
                    return Err(anyhow!(
                        "Could not convert section offset to debug info offset"
                    ))
                }
            })?;
    let unit = gimli::Unit::new(dwarf, header)?;
    let mut tree = unit.entries_tree(Some(unit_offset))?;
    let node = tree.root()?;

    let die = unit.entry(unit_offset)?;
    // Get the name of the function.
    let name = match die.attr_value(gimli::DW_AT_name)? {
        Some(DebugStrRef(offset)) => format!("{:?}", dwarf.string(offset)?.to_string()?),
        _ => "<unknown>".to_string(),
    };

    // Get source information about the function
    let source = SourceInformation::get_die_source_information(dwarf, &unit, &node.entry(), cwd)?;

    // Get all the variable dies to evaluate.
    let dies_to_check = get_functions_variables_die_offset(
        dwarf,
        section_offset,
        unit_offset,
        call_frame.code_location as u32,
    )?;

    // Get register values
    let mut temporary_registers = Registers::new();
    temporary_registers.program_counter_register = registers.program_counter_register;
    temporary_registers.link_register = registers.link_register;
    temporary_registers.stack_pointer_register = registers.stack_pointer_register;
    temporary_registers.cfa = call_frame.cfa;
    let pc = call_frame.code_location as u32;
    for i in 0..call_frame.registers.len() {
        match call_frame.registers[i] {
            Some(val) => temporary_registers.add_register_value(i as u16, val),
            None => (),
        };
    }

    let frame_base = evaluate_frame_base(dwarf, &unit, pc, &die, &mut temporary_registers, mem)?;

    let mut variables = vec![];

    for variable_die in dies_to_check {
        let vc = Variable::get_variable(
            dwarf,
            &temporary_registers,
            mem,
            section_offset,
            variable_die,
            Some(frame_base),
            cwd,
        )?;

        variables.push(vc);
    }

    Ok(StackFrame {
        call_frame,
        name,
        source,
        variables,
    })
}

pub fn find_function_die<'a, R: Reader<Offset = usize>>(
    dwarf: &'a Dwarf<R>,
    address: u32,
) -> Result<(gimli::UnitSectionOffset, gimli::UnitOffset)> {
    let unit = get_current_unit(&dwarf, address)?;
    let mut cursor = unit.entries();

    let mut depth = 0;
    let mut res = None;
    let mut dies = vec![];

    assert!(cursor.next_dfs()?.is_some());
    while let Some((delta_depth, current)) = cursor.next_dfs()? {
        // Update depth value, and break out of the loop when we
        // return to the original starting position.
        depth += delta_depth;
        if depth <= 0 {
            break;
        }

        match current.tag() {
            gimli::DW_TAG_subprogram | gimli::DW_TAG_inlined_subroutine => {
                if let Some(true) = die_in_range(&dwarf, &unit, current, address) {
                    match res {
                        Some(val) => {
                            if val > depth {
                                res = Some(depth);
                                dies = vec![current.clone()];
                            } else if val == depth {
                                dies.push(current.clone());
                            }
                        }
                        None => {
                            res = Some(depth);
                            dies.push(current.clone());
                        }
                    };
                }
            }
            _ => (),
        };
    }

    if dies.len() != 1 {
        unreachable!();
    }
    return Ok((unit.header.offset(), dies[0].offset()));
}

pub fn get_functions_variables_die_offset<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    section_offset: UnitSectionOffset,
    unit_offset: UnitOffset,
    pc: u32,
) -> Result<Vec<UnitOffset>> {
    fn recursive_offset<R: Reader<Offset = usize>>(
        dwarf: &Dwarf<R>,
        unit: &Unit<R>,
        node: EntriesTreeNode<R>,
        pc: u32,
        list: &mut Vec<UnitOffset>,
    ) -> Result<()> {
        let die = node.entry();

        match die_in_range(dwarf, unit, die, pc) {
            Some(false) => return Ok(()),
            _ => (),
        };

        if is_variable_die(&die) {
            list.push(die.offset());
        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            recursive_offset(dwarf, unit, child, pc, list)?;
        }

        Ok(())
    }

    let header =
        dwarf
            .debug_info
            .header_from_offset(match section_offset.as_debug_info_offset() {
                Some(val) => val,
                None => {
                    return Err(anyhow!(
                        "Could not convert section offset to debug info offset"
                    ))
                }
            })?;
    let unit = gimli::Unit::new(dwarf, header)?;
    let mut tree = unit.entries_tree(Some(unit_offset))?;
    let node = tree.root()?;

    let mut die_offsets = vec![];

    // Recursively process the children.
    let mut children = node.children();
    while let Some(child) = children.next()? {
        recursive_offset(dwarf, &unit, child, pc, &mut die_offsets)?;
    }

    Ok(die_offsets)
}

pub fn evaluate_frame_base<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    pc: u32,
    die: &DebuggingInformationEntry<'_, '_, R>,
    registers: &mut Registers,
    mem: &mut T,
) -> Result<u64> {
    if let Some(val) = die.attr_value(gimli::DW_AT_frame_base)? {
        if let Some(expr) = val.exprloc_value() {
            let value = evaluate(
                dwarf,
                unit,
                pc,
                expr.clone(),
                None,
                None,
                None,
                registers,
                mem,
            )?;

            match value {
                EvaluatorValue::Value(BaseValue::Address32(v), _) => return Ok(v as u64),
                _ => {
                    unreachable!();
                }
            };
        } else {
            unimplemented!();
        }
    } else {
        return Err(anyhow!("Die has no DW_AT_frame_base attribute"));
    }
}
