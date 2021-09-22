/// Contains functions for retrieving the values of some of the DWARF attributes.
pub mod attributes;

/// Contains structs representing the different Rust data types and more.
pub mod evaluate;

/// Contains a function for evaluating a DWARF expression into a `Vec` of `Piece`s.
pub mod pieces;

use crate::call_stack::MemoryAccess;
use crate::evaluate::pieces::evaluate_pieces;
use crate::registers::Registers;
use anyhow::{bail, Result};
use evaluate::EvaluatorValue;
use gimli::{AttributeValue::UnitRef, DebuggingInformationEntry, Dwarf, Expression, Reader, Unit};

/// Will find the DIE representing the type can evaluate the variable.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `pc` - A machine code address, usually the current code location.
/// * `expr` - The expression to be evaluated.
/// * `frame_base` - The frame base address value.
/// * `unit` - A compilation unit which contains the given DIE.
/// * `die` - The DIE the is used to find the DIE representing the type.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// This function is used to find the DIE representing the type and then to evaluate the value of
/// the given DIE>
pub fn call_evaluate<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    pc: u32,
    expr: gimli::Expression<R>,
    frame_base: Option<u64>,
    unit: &Unit<R>,
    die: &DebuggingInformationEntry<R>,
    registers: &Registers,
    mem: &mut T,
) -> Result<EvaluatorValue<R>> {
    if let Ok(Some(tattr)) = die.attr_value(gimli::DW_AT_type) {
        match tattr {
            gimli::AttributeValue::UnitRef(offset) => {
                let die = unit.entry(offset)?;
                return evaluate(
                    dwarf,
                    unit,
                    pc,
                    expr,
                    frame_base,
                    Some(unit),
                    Some(&die),
                    registers,
                    mem,
                );
            }
            gimli::AttributeValue::DebugInfoRef(di_offset) => {
                let offset = gimli::UnitSectionOffset::DebugInfoOffset(di_offset);
                let mut iter = dwarf.debug_info.units();
                while let Ok(Some(header)) = iter.next() {
                    let type_unit = dwarf.unit(header)?;
                    if let Some(offset) = offset.to_unit_offset(&type_unit) {
                        let die = type_unit.entry(offset)?;
                        return evaluate(
                            dwarf,
                            unit,
                            pc,
                            expr,
                            frame_base,
                            Some(&type_unit),
                            Some(&die),
                            registers,
                            mem,
                        );
                    }
                }
                bail!("");
            }
            _ => bail!(""),
        };
    } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
        match die_offset {
            UnitRef(offset) => {
                if let Ok(ndie) = unit.entry(offset) {
                    return call_evaluate(dwarf, pc, expr, frame_base, unit, &ndie, registers, mem);
                }
            }
            _ => {
                unimplemented!();
            }
        };
    }
    bail!("");
}

/// Will evaluate the value of the given DWARF expression.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A compilation unit which contains the given DIE.
/// * `pc` - A machine code address, usually the current code location.
/// * `expr` - The expression to be evaluated.
/// * `frame_base` - The frame base address value.
/// * `type_unit` - A compilation unit which contains the given DIE which represents the type of
/// the given expression. None if the expression does not have a type.
/// * `type_die` - The DIE the represents the type of the given expression. None if the expression
/// does not have a type.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// This function will first evaluate the expression into gimli-rs `Piece`s.
/// Then it will use the pieces and the type too evaluate and parse the value.
pub fn evaluate<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    pc: u32,
    expr: Expression<R>,
    frame_base: Option<u64>,
    type_unit: Option<&gimli::Unit<R>>,
    type_die: Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
    registers: &Registers,
    mem: &mut T,
) -> Result<EvaluatorValue<R>> {
    let pieces = evaluate_pieces(dwarf, unit, pc, expr, frame_base, registers, mem)?;
    evaluate_value(dwarf, pieces, type_unit, type_die, registers, mem)
}

/// Will evaluate the value of the given list of gimli-rs `Piece`s.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `pieces` - A list of gimli-rs pieces containing the location information..
/// * `type_unit` - A compilation unit which contains the given DIE which represents the type of
/// the given expression. None if the expression does not have a type.
/// * `type_die` - The DIE the represents the type of the given expression. None if the expression
/// does not have a type.
/// * `registers` - A register struct for accessing the register values.
/// * `mem` - A struct for accessing the memory of the debug target.
///
/// Then it will use the pieces and the type too evaluate and parse the value.
pub fn evaluate_value<R: Reader<Offset = usize>, T: MemoryAccess>(
    dwarf: &Dwarf<R>,
    pieces: Vec<gimli::Piece<R>>,
    type_unit: Option<&gimli::Unit<R>>,
    type_die: Option<&gimli::DebuggingInformationEntry<'_, '_, R>>,
    registers: &Registers,
    mem: &mut T,
) -> Result<EvaluatorValue<R>> {
    match type_unit {
        Some(unit) => match type_die {
            Some(die) => {
                return EvaluatorValue::evaluate_variable_with_type(
                    dwarf,
                    registers,
                    mem,
                    &pieces,
                    unit.header.offset(),
                    die.offset(),
                );
            }
            None => (),
        },
        None => (),
    };
    return EvaluatorValue::evaluate_variable(registers, mem, &pieces);
}
