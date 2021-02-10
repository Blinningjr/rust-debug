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
            node: EntriesTreeNode<R>,
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

        // only look at dies that are in range.
        if !in_r {
            return Ok(false);
        }

        println!("in_r: {:?}", in_r);
        if let Some(fb) = self.check_die(die, frame_base) {
            frame_base = Some(fb);
        }
        if die.tag() == gimli::DW_TAG_variable {
            if let Some(name) =  die.attr_value(gimli::DW_AT_name)? {
                if let DebugStrRef(offset) = name  {
                    if self.dwarf.string(offset).unwrap().to_string().unwrap() == "test_enum" {
                        
                        if let Some(UnitRef(offset)) =  die.attr_value(gimli::DW_AT_type)? {
                            println!("{:#?}", offset);
                            self.check_die(&self.unit.entry(offset)?, frame_base);
                        }
                        

                        return Ok(true);
                    }
                }
            }
        }
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
                    frame_base = match self.evaluate(&self.unit, expr, frame_base, None).unwrap() {
                        DebuggerValue::Value(Value::U64(v)) => Some(v),
                        DebuggerValue::Value(Value::U32(v)) => Some(v as u64),
                        _ => frame_base,
                    };
                } else {
                    let tdie = match die.attr_value(gimli::DW_AT_type).unwrap().unwrap() {
                        UnitRef(offset) => self.unit.entry(offset).unwrap(),
                        _ => unimplemented!(),
                    };
                    self.evaluate(self.unit, expr, frame_base, Some(&tdie));
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

