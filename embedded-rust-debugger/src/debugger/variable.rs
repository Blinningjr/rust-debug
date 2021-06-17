use anyhow::Result;

use gimli::{
    Reader,
    Dwarf,
    Unit,
    DebuggingInformationEntry,
    EntriesTreeNode,
    AttributeValue::DebugStrRef,
    UnitSectionOffset,
    UnitOffset,
};


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
}

impl VariableCreator {
    pub fn new<R: Reader<Offset = usize>>(dwarf: &Dwarf<R>,
               section_offset: UnitSectionOffset,
               unit_offset: UnitOffset
               ) -> Result<VariableCreator>
    {
        let (section_offset, unit_offset) = find_function_die(dwarf, call_frame.code_location as u32)?;
        let header = dwarf.debug_info.header_from_offset(section_offset.as_debug_info_offset().unwrap())?;
        let unit = gimli::Unit::new(dwarf, header)?;
        let die = unit.entry(unit_offset);


        let name = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => format!("{:?}", dwarf.string(offset)?.to_string()?),
            _ => "<unknown>".to_string(),
        };

        Ok(VariableCreator {
            section_offset: section_offset,
            unit_offset: unit_offset,
            name: name,
            value: None,
        })
    }


    pub fn get_variable(&self) -> Result<Variable> {
        match self.value {
            Some(val) => Variable {
                name: self.name.clone(),
                value: val,
            },
            None => Err(anyhow!("Variables location not evaluated yet")),
        }
    }


    pub fn continue_create(&mut self, dwarf: &Dwarf<R>) -> Result<bool> {
        unimplemented!();
    }
}

