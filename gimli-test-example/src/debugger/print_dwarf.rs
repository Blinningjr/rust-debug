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
};


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn print_tree(&mut self, 
                      node: EntriesTreeNode<R>
                      ) -> gimli::Result<()>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, &self.unit, die, self.pc) {
            Some(false) =>(),// return Ok(()),
            _ => (),
        };

        self.print_die(die)?;

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            self.print_tree(child)?
        }
        return Ok(());
    }
    
   
    pub fn print_die(&mut self,
                     die: &DebuggingInformationEntry<'_, '_, R>
                     ) -> gimli::Result<()>
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

