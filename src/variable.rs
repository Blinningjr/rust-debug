use gimli::{
    Reader,
    Dwarf,
    Unit,
    DebuggingInformationEntry,
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
    bail,
    anyhow,
};

use crate::call_stack::MemoryAccess;
use crate::evaluate::value_information::ValueInformation;
use crate::source_information::SourceInformation;
use crate::evaluate::attributes;
use crate::utils::in_range;
use crate::evaluate::EvalResult;
use crate::evaluate::EvaluatorResult;
use crate::evaluate::evaluate;
use crate::registers::Registers;


#[derive(Debug, Clone)]
pub struct Variable {
    pub name:   Option<String>,
    pub value:  String,
    pub type_:  Option<String>,
//    pub locations: Vec<u32>, // u32 or registery number
    pub source: Option<SourceInformation>,
    pub location: Vec<ValueInformation>,
}


#[derive(Debug, Clone)]
pub struct VariableCreator {
    pub section_offset: UnitSectionOffset,
    pub unit_offset: UnitOffset,
    pub name:   Option<String>,
    pub source: Option<SourceInformation>,
    pub value:  Option<String>,
    pub type_:  Option<String>,
    pub frame_base: Option<u64>,
    pub pc: u32,

    pub var_info: Option<Vec<ValueInformation>>,
}


impl VariableCreator {
    pub fn new<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
               section_offset: UnitSectionOffset,
               unit_offset: UnitOffset,
               frame_base: Option<u64>,
               pc: u32,
               cwd: &str,
               ) -> Result<VariableCreator>
    {
        let header = dwarf.debug_info.header_from_offset(match section_offset.as_debug_info_offset() {
            Some(val) => val,
            None => bail!("Could not convert section offset into debug info offset"),
        })?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let die = unit.entry(unit_offset)?;

        let name = get_var_name(dwarf, &unit, &die)?;

        let source = match find_variable_source_information(dwarf, &unit, &die, cwd) {
            Ok(source) => Some(source),
            Err(_) => None,
        };

        Ok(VariableCreator {
            section_offset: section_offset,
            unit_offset: unit_offset,
            name: name,
            source: source,
            value: None,
            type_: None,
            frame_base: frame_base,
            pc: pc,
            var_info: None,
        })
    }


    pub fn get_variable(&self) -> Result<Variable> {
        match (&self.value, &self.var_info) {
            (Some(val), Some(var_info)) => Ok(Variable {
                name: self.name.clone(),
                value: val.clone(),
                type_: self.type_.clone(),
                source: self.source.clone(),
                location: var_info.clone(),
            }),
            (Some(val), None) => Ok(Variable {
                name: self.name.clone(),
                value: val.clone(),
                type_: self.type_.clone(),
                source: self.source.clone(),
                location: vec!(),
            }),
            _ => Err(anyhow!("Variable has not been evaluated yet")),
        }
    }


    pub fn continue_create<R: Reader<Offset = usize>, T: MemoryAccess>(&mut self,
                                                      dwarf: &Dwarf<R>,
                                                      registers: &Registers,
                                                        mem:                         &mut T,
                                                      ) -> Result<EvalResult> {
        let header = dwarf.debug_info.header_from_offset(match self.section_offset.as_debug_info_offset() {
            Some(val) => val,
            None => bail!("Could not convert the section offset into debug info offset"),
        })?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let die = unit.entry(self.unit_offset)?;


        let expression = match find_variable_location(dwarf, &unit, &die, self.pc)? {
            VariableLocation::Expression(expr) => expr,
            VariableLocation::LocationListEntry(llent) => llent.data,
            VariableLocation::OutOfRange => {
                self.value = Some("<OutOfRange>".to_owned());
                return Ok(EvalResult::Complete);
            },
            VariableLocation::NoLocation => {
                self.value = Some("<OptimizedOut>".to_owned());
                return Ok(EvalResult::Complete);
            },
        };


        let (type_section_offset, type_unit_offset) = find_variable_type_die(dwarf, &unit, &die)?;

        let header = dwarf.debug_info.header_from_offset(match type_section_offset.as_debug_info_offset() {
            Some(val) => val,
            None => bail!("Could not convert the section offset into debug info offset"),
        })?;
        let type_unit = gimli::Unit::new(dwarf, header)?;
        let type_die = unit.entry(type_unit_offset)?;


        match evaluate(dwarf,
                 &unit,
                 self.pc,
                 expression,
                 self.frame_base,
                 Some(&type_unit),
                 Some(&type_die),
                 registers,
                 mem)? {
            EvaluatorResult::Complete(val) => {
                self.value = Some(val.to_string()); 
                self.type_ = Some(val.get_type()); 
                self.var_info = Some(val.get_variable_information());
                Ok(EvalResult::Complete)
            },
            EvaluatorResult::Requires(req) => Ok(req),
        }
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
                                           ) -> Result<Option<String>>
{
    if is_variable_die(die) {
        // Get the name of the variable.
        if let Ok(Some(DebugStrRef(offset))) =  die.attr_value(gimli::DW_AT_name) {
            return Ok(Some(
                dwarf.string(offset)?.to_string()?.to_string()
            ));

        } else if let Ok(Some(offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match offset {
                UnitRef(o) => {
                    if let Ok(ndie) = unit.entry(o) {
                        return get_var_name(dwarf, unit, &ndie);
                    }
                },
                _ => {
                    unimplemented!();
                },
            };
        }

        return Ok(None);
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
                bail!("Unimplemented for {:?}", v);
            },
        }
    } else {
        return Err(anyhow!("This die is not a variable"));
    } 
}


pub fn find_variable_type_die<R: Reader<Offset = usize>>(dwarf:    & Dwarf<R>,
                          unit:         &Unit<R>,
                          die:          &DebuggingInformationEntry<R>,
                          ) -> Result<(UnitSectionOffset, UnitOffset)>
{
    if is_variable_die(die) {
        match attributes::type_attribute(dwarf, unit, die)? {
            Some(result) => return Ok(result),
            None => {
                if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
                    match die_offset {
                        UnitRef(offset) => {
                            if let Ok(ao_die) = unit.entry(offset) {
                                return find_variable_type_die(dwarf, unit, &ao_die);
                            }
                        },
                        _ => {
                            unimplemented!();
                        },
                    };        
                }

                return Err(anyhow!("Could not find this variables type die"));
            },
        }
    } else {
        return Err(anyhow!("This die is not a variable"));
    }
}


pub fn find_variable_source_information<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>, unit: &Unit<R>, die: &DebuggingInformationEntry<R>, cwd: &str) -> Result<SourceInformation>
{
    if is_variable_die(die) {
        if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match die_offset {
                UnitRef(offset) => {
                    let ao_die = unit.entry(offset)?;
                    return find_variable_source_information(dwarf, unit, &ao_die, cwd);
                },
                _ => {
                    unimplemented!();
                },
            };
        } else {
            return SourceInformation::get_die_source_information(dwarf, unit, die, cwd);
        }
    } else {
        return Err(anyhow!("This die is not a variable"));
    }
}

