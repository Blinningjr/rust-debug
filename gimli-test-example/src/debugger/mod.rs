mod evaluate;

use evaluate::{
    DebuggerValue,
};

use super::{
    in_range,
};

use probe_rs::{
    Core,
};

use gimli::{
    Unit,
    Dwarf,
    DebuggingInformationEntry,
    AttributeValue::{
        DebugStrRef,
        UnitRef,
    },
    Reader,
    EntriesTreeNode,
    Value,
    Error,
};

pub struct Debugger<'a, R: Reader<Offset = usize>> {
    core: Core<'a>,
    dwarf: Dwarf<R>,
    unit: &'a Unit<R>,
    pc: u32,
}

impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn new(core: Core<'a>,
               dwarf: Dwarf<R>,
               unit: &'a Unit<R>,
               pc: u32) -> Debugger<'a, R> {
        Debugger{
            core: core,
            dwarf: dwarf,
            unit: unit,
            pc: pc,
        }
    }

    pub fn find_variable(&mut self, search: &str) -> gimli::Result<DebuggerValue<R>> {
        let mut tree = self.unit.entries_tree(None)?;
        let root = tree.root()?;
        return match self.process_tree(root, None, search)? {
            Some((val, _)) => Ok(val),
            None => Err(Error::Io),
        };
    }


    pub fn process_tree(&mut self, 
            node: EntriesTreeNode<R>,
            mut frame_base: Option<u64>,
            search: &str
        ) -> gimli::Result<Option<(DebuggerValue<R>, Option<String>)>>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, &self.unit, die, self.pc) {
            Some(false) => return Ok(None),
            _ => (),
        };

        frame_base = self.check_frame_base(&die, frame_base)?;

        // Check for the searched vairable.
        if die.tag() == gimli::DW_TAG_variable { // Check that it is a variable.
            if let Some(DebugStrRef(offset)) =  die.attr_value(gimli::DW_AT_name)? { // Get the name of the variable.
                if self.dwarf.string(offset).unwrap().to_string().unwrap() == search { // Compare the name of the variable.

                    if let Some(UnitRef(offset)) =  die.attr_value(gimli::DW_AT_type)? { 
                        println!("\n");
                        let value =self.check_die(&die, frame_base).unwrap();
                        println!("\n");

                        let tdie = self.unit.entry(offset)?;
                        let name = match tdie.attr_value(gimli::DW_AT_name).unwrap().unwrap() {
                            DebugStrRef(offset) => self.dwarf.string(offset).unwrap().to_string().unwrap().to_string(),
                            _ => panic!("error"),
                        };
                        self.check_die(&tdie, frame_base);

                        return Ok(Some((value, Some(name))));
                    }

                }
            }
        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            if let Some(result) = self.process_tree(child, frame_base, search)? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }


    pub fn check_frame_base(&mut self,
                            die: &DebuggingInformationEntry<'_, '_, R>,
                            frame_base: Option<u64>
                            ) -> gimli::Result<Option<u64>>
    {
        if let Some(val) = die.attr_value(gimli::DW_AT_frame_base)? {
            if let Some(expr) = val.exprloc_value() {
                return Ok(match self.evaluate(&self.unit, expr, frame_base, None).unwrap() {
                    DebuggerValue::Value(Value::U64(v)) => Some(v),
                    DebuggerValue::Value(Value::U32(v)) => Some(v as u64),
                    _ => frame_base,
                });
            } else {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }
    }


    pub fn check_die(&mut self,
                     die: &DebuggingInformationEntry<'_, '_, R>,
                     mut frame_base: Option<u64>
        ) -> Option<DebuggerValue<R>>
    {
    
        let mut attrs = die.attrs();
        println!("{:?}", die.tag().static_string());
        println!(
            "{:<30} | {:<}",
            "Name", "Value"
        );
        println!("----------------------------------------------------------------");
        while let Some(attr) = attrs.next().unwrap() {
            let val = match attr.value() {
                DebugStrRef(offset) => format!("{:?}", self.dwarf.string(offset).unwrap().to_string().unwrap()),
                _ => format!("{:?}", attr.value()),
            };
    
            println!(
                "{: <30} | {:<?}",
                attr.name().static_string().unwrap(),
                val
            );
            if let Some(expr) = attr.value().exprloc_value() {
                let tdie = match die.attr_value(gimli::DW_AT_type).unwrap().unwrap() {
                    UnitRef(offset) => self.unit.entry(offset).unwrap(),
                    _ => unimplemented!(),
                };
                return Some(self.evaluate(self.unit, expr, frame_base, Some(&tdie)).unwrap());
            }
        }
        println!("\n");
    
        return None;
    }
}


fn die_in_range<'a, R>(
        dwarf: &'a Dwarf<R>,
        unit: &'a Unit<R>,
        die: &DebuggingInformationEntry<'_, '_, R>,
        pc: u32,)
    -> Option<bool>
        where R: Reader<Offset = usize>
{
    match dwarf.die_ranges(unit, die) {
        Ok(mut range) => in_range(pc, &mut range),
        Err(_) => None,
    }
}

