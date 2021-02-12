use super::{
    Debugger,
    DebuggerValue,
    utils::{
        die_in_range,
    },
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


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn print_tree(&mut self, 
            node: EntriesTreeNode<R>,
            mut frame_base: Option<u64>
        ) -> gimli::Result<()>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, &self.unit, die, self.pc) {
            Some(false) => return Ok(()),
            _ => (),
        };

        frame_base = self.check_frame_base(&die, frame_base)?;
        self.print_die(die, frame_base);


        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            self.print_tree(child, frame_base)?
        }
        return Ok(());
    }
    
   
    pub fn print_die(&mut self,
                     die: &DebuggingInformationEntry<'_, '_, R>,
                     frame_base: Option<u64>
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
                let dtype = match die.attr_value(gimli::DW_AT_type).unwrap() {
                    Some(attr) => self.parse_type_attr(attr).unwrap(),
                    _ => unimplemented!(),
                };
                return Some(self.evaluate(self.unit, expr, frame_base, Some(&dtype)).unwrap());
            }
        }
        println!("\n");
    
        return None;
    }
}

