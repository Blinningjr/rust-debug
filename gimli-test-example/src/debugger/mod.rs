mod evaluate;

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
    },
    Reader,
    EntriesTreeNode,
    Value,
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

    pub fn process_tree(&mut self, 
            mut node: EntriesTreeNode<R>,
            prev_in_range: bool,
            mut frame_base: Option<u64>
        ) -> gimli::Result<bool>
    {
        let die = node.entry();
        let in_range = die_in_range(&self.dwarf, &self.unit, die, self.pc);
        let mut in_r = true;
        match (in_range, prev_in_range) {
            (Some(false), _ ) => in_r = false, //return Ok(()),
            (None, false) => in_r = false, //return Ok(()),
            _ => (),
        };
        println!("in_r: {:?}", in_r);
        if let Some(fb) = self.check_die(die, frame_base) {
            frame_base = Some(fb);
        }
//        if die.tag() == gimli::DW_TAG_variable {
//            if let Some(name) =  die.attr_value(gimli::DW_AT_name)? {
//                if let DebugStrRef(offset) = name  {
//                    if dwarf.string(offset).unwrap().to_string().unwrap() == "my_num" {
//                        return Ok(true);
//                    }
//                }
//            }
//        }
        if in_r {
            let mut children = node.children();
            while let Some(child) = children.next()? {
                // Recursively process a child.
                if self.process_tree(child, in_r, frame_base)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn check_die(&mut self,
                     die: &DebuggingInformationEntry<'_, '_, R>,
                     mut frame_base: Option<u64>
        ) -> Option<u64>
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
                if attr.name() == gimli::DW_AT_frame_base {
                    frame_base = match self.new_evaluate(&self.unit, expr, frame_base).unwrap() {
                        Value::U64(v) => Some(v),
                        Value::U32(v) => Some(v as u64),
                        _ => frame_base,
                    };
                } else {
                    self.new_evaluate(self.unit, expr, frame_base);
                }
            }
        }
        println!("\n");
    
        return frame_base;
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

