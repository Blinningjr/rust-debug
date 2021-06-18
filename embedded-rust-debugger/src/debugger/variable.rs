use gimli::{
    Reader,
    Dwarf,
    Unit,
    DebuggingInformationEntry,
    EntriesTreeNode,
    AttributeValue::{
        DebugStrRef,
        UnitRef,
        Exprloc,
        LocationListsRef,
    },
    UnitSectionOffset,
    UnitOffset,
};

use anyhow::{
    Result,
    anyhow,
};

use std::collections::HashMap;

use crate::debugger::stack_frame::find_function_die;
use crate::debugger::in_range;


#[derive(Debug, Clone)]
pub struct Variable {
    pub name:   String,
    pub value:  String,
//    pub type_:  String,
//    pub locations: Vec<u32>, // u32 or registery number
//    pub source: Source,
}


#[derive(Debug, Clone)]
pub struct VariableCreator {
    pub section_offset: UnitSectionOffset,
    pub unit_offset: UnitOffset,
    pub name:   String,
    pub value:  Option<String>,
    pub frame_base: Option<u64>,
    pub pc: u32,

    pub registers: HashMap<u16, u32>,
    pub memory: HashMap<u32, u32>,
}


impl VariableCreator {
    pub fn new<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
               section_offset: UnitSectionOffset,
               unit_offset: UnitOffset,
               registers: Vec<(u16, u32)>,
               frame_base: Option<u64>,
               pc: u32,
               ) -> Result<VariableCreator>
    {
        let header = dwarf.debug_info.header_from_offset(section_offset.as_debug_info_offset().unwrap())?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let die = unit.entry(unit_offset)?;

        let name = get_var_name(dwarf, &unit, &die)?;

        let mut regs = HashMap::new();
        for (reg, val) in registers {
            regs.insert(reg, val);
        }

        Ok(VariableCreator {
            section_offset: section_offset,
            unit_offset: unit_offset,
            name: name,
            value: None,
            frame_base: frame_base,
            pc: pc,

            registers: regs,
            memory: HashMap::new(),
        })
    }


    pub fn get_variable(&self) -> Result<Variable> {
        match &self.value {
            Some(val) => Ok(Variable {
                name: self.name.clone(),
                value: val.clone(),
            }),
            None => Err(anyhow!("Variables location not evaluated yet")),
        }
    }


    pub fn continue_create<R: Reader<Offset = usize>>(&mut self, dwarf: &Dwarf<R>) -> Result<bool> {
        unimplemented!();
    }
}


pub fn is_variable_die<R: Reader<Offset = usize>>(die: &DebuggingInformationEntry<R>) -> bool {
    // Check that it is a variable.
    return die.tag() == gimli::DW_TAG_variable ||
        die.tag() == gimli::DW_TAG_formal_parameter ||
        die.tag() == gimli::DW_TAG_constant; 
}


fn get_var_name<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                           unit:     &Unit<R>,
                                           die:      &DebuggingInformationEntry<R>,
                                           ) -> Result<String>
{
    if is_variable_die(die) {
        // Get the name of the variable.
        if let Ok(Some(DebugStrRef(offset))) =  die.attr_value(gimli::DW_AT_name) {
            return Ok(
                dwarf.string(offset)?.to_string()?.to_string()
            );

        } else if let Ok(Some(offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match offset {
                UnitRef(o) => {
                    if let Ok(ndie) = unit.entry(o) {
                        return get_var_name(dwarf, unit, &ndie);
                    }
                },
                _ => {
                    println!("{:?}", offset);
                    unimplemented!();
                },
            };
        }

        return Err(anyhow!("Can't find name attribute"));
    } else {
        return Err(anyhow!("This die is not a variable"));
    }
}



pub enum VariableLocation<R: Reader<Offset = usize>> {
    Expression(gimli::Expression<R>),
    LocationListEntry(gimli::LocationListEntry<R>),
    OutOfRange,
    NoLocation,
}

pub fn find_variable_location<R: Reader<Offset = usize>>(dwarf:    & Dwarf<R>,
                          unit:         &Unit<R>,
                          die:          &DebuggingInformationEntry<R>,
                          pc:           u32,
                          ) -> Result<VariableLocation<R>>
{
    if is_variable_die(die) {
        match die.attr_value(gimli::DW_AT_location)? {
            Some(Exprloc(expr)) => {
                return Ok(VariableLocation::Expression(expr));
            },
            Some(LocationListsRef(offset)) => {
                let mut locations = dwarf.locations(unit, offset)?;
                while let Some(llent) = locations.next()? {
                    if in_range(pc, &llent.range) {
                        return Ok(VariableLocation::LocationListEntry(llent));
                    }
                }

                return Ok(VariableLocation::OutOfRange);
            },
            None => return Ok(VariableLocation::NoLocation),
            Some(v) => {
                println!("{:?}", v);
                unimplemented!();
            },
        }
    } else {
        return Err(anyhow!("This die is not a variable"));
    } 
}

