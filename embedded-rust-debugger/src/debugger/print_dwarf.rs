use super::{
    Debugger,
    utils::{
        die_in_range,
    },
};

use gimli::{
    DebuggingInformationEntry,
    AttributeValue::{
        DebugStrRef,
    },
    Reader,
    EntriesTreeNode,
    Unit,
};


use anyhow::{
    Result,
};


impl<R: Reader<Offset = usize>> Debugger<R> {
    pub fn print_tree(&mut self, 
                         unit:      &Unit<R>,
                         pc:        u32,
                      node: EntriesTreeNode<R>
                      ) -> Result<()>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, unit, die, pc) {
            Some(false) =>(),// return Ok(()),
            _ => (),
        };

        self.print_die(die)?;

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            self.print_tree(unit, pc, child)?
        }
        return Ok(());
    }
    
   
    pub fn print_die(&mut self,
                     die: &DebuggingInformationEntry<'_, '_, R>
                     ) -> Result<()>
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
                DebugStrRef(offset) => format!("{:?}", self.dwarf.string(offset)?.to_string()?),
                _ => format!("{:?}", attr.value()),
            };
    
            println!(
                "{: <30} | {:<x?}",
                attr.name().static_string().unwrap(),
                val
            );

            if let Some(_) = attr.value().exprloc_value() {
                //panic!("Found value");
            }
        }
        println!("\n");
        return Ok(());
    }
}

