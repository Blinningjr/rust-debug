pub mod types;
mod parser;

pub use types::{
    TypeInfo,
    DebuggerType,
};

use super::{
    Debugger,
    Reader,
    utils::{
        die_in_range,
    },
};

use gimli::{
    EntriesTreeNode,
    AttributeValue::{
        DebugStrRef,
    },
    Unit,
};

use anyhow::{
    Result,
};


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn find_type(&mut self,
                         unit:      &Unit<R>,
                         pc:        u32,
                     search: &str
                     ) -> Result<()> {
        let mut tree    = unit.entries_tree(None)?;
        let root        = tree.root()?;

        self.process_tree_type(unit, pc, root, None, search)?;
        return Ok(());
    }


    pub fn process_tree_type(&mut self,
                         unit:      &Unit<R>,
                         pc:        u32,
                             node: EntriesTreeNode<R>,
                             mut frame_base: Option<u64>,
                             search: &str
                             ) -> Result<bool>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, unit, die, pc) {
            Some(false) => return Ok(false),
            _ => (),
        };

        frame_base = self.check_frame_base(unit, pc, &die, frame_base)?;

        // Check for the searched type.
        if let Some(DebugStrRef(offset)) =  die.attr_value(gimli::DW_AT_name)? { // Get the name of the variable.
            if self.dwarf.string(offset)?.to_string()? == search { // Compare the name of the variable.
                self.print_tree(unit, pc, node)?;

                // Recursively process the children.
                //let mut i = 0;
                //let mut children = node.children();
                //while let Some(child) = children.next()? {
                //    if i == -1 {
                //        self.print_tree(unit, pc, child)?;
                //    }

                //    i += 1;
                //}

                return Ok(true);
            }
        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            if self.process_tree_type(unit, pc, child, frame_base, search)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

