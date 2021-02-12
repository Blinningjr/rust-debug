pub mod utils;
pub mod print_dwarf;
pub mod evaluate;
pub mod type_parser;


use utils::{
    in_range,
    die_in_range,
};


use evaluate::{
    DebuggerValue,
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
                        let value =self.print_die(&die, frame_base).unwrap();
                        println!("\n");

                        let mut tree = self.unit.entries_tree(Some(offset))?;
                        let root = tree.root()?;
                        self.print_tree(root, frame_base);

                        return Ok(Some((value, None)));
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
}



