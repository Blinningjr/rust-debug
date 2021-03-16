pub mod utils;
pub mod print_dwarf;
pub mod evaluate;
pub mod types;
//pub mod type_value;
pub mod attributes;


use utils::{
    die_in_range,
    in_range,
};
use crate::debugger::types::DebuggerType;


use evaluate::value::{
    DebuggerValue,
    Value,
};


use probe_rs::{
    Core,
};


use anyhow::{
    Result,
    anyhow,
};

use gimli::{
    Unit,
    Dwarf,
    DebuggingInformationEntry,
    AttributeValue::{
        DebugStrRef,
        Exprloc,
        LocationListsRef,
        UnitRef,
    },
    Reader,
    EntriesTreeNode,
};


pub struct Debugger<'a, R: Reader<Offset = usize>> {
    pub core:   Core<'a>,
    pub dwarf:  Dwarf<R>,
}


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn new(core:    Core<'a>,
               dwarf:   Dwarf<R>,
               ) -> Debugger<'a, R> {
        Debugger{
            core:   core,
            dwarf:  dwarf,
        }
    }


    pub fn find_variable(&mut self,
                         unit:      &Unit<R>,
                         pc:        u32,
                         search:    &str
                         ) -> Result<DebuggerValue<R>>
    {
        let mut tree    = unit.entries_tree(None)?;
        let root        = tree.root()?;

//        self.print_tree(root)?;
//        unimplemented!();

        return match self.process_tree(unit, pc, root, None, search)? {
            Some(val)   => Ok(val),
            None        => Err(anyhow!("Can't find value")), // TODO: Change to a better error.
        };
    }


    pub fn process_tree(&mut self, 
                        unit:           &Unit<R>,
                        pc:             u32,
                        node:           EntriesTreeNode<R>,
                        mut frame_base: Option<u64>,
                        search:         &str
                        ) -> Result<Option<DebuggerValue<R>>>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, unit, die, pc) {
            Some(false) => return Ok(None),
            _ => (),
        };

        frame_base = self.check_frame_base(unit, pc, &die, frame_base)?;

        // Check for the searched vairable.
        if self.check_var_name(unit, pc, &die, search) {
            //println!("\n");
            //self.print_die(&die)?;
            let dtype = self.get_var_type(unit, pc, &die).unwrap();
            println!("{:#?}", dtype);
            match self.eval_location(unit, pc, &die, &dtype, frame_base) {
                Ok(v) => return Ok(v),
                Err(_) => (),
            };
        }
        
//        //self.print_die(&die)?;
//        if let Some(dtype) = self.get_var_type(unit, pc, &die) {
//            //println!("{:#?}", dtype);
//            //self.print_die(&die)?;
//            self.eval_location(unit, pc, &die, &dtype, frame_base);
//        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            if let Some(result) = self.process_tree(unit, pc, child, frame_base, search)? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }


    fn check_var_name(&mut self,
                      unit:     &Unit<R>,
                      pc:       u32,
                      die:      &DebuggingInformationEntry<R>,
                      search:   &str
                      ) -> bool
    {
        if die.tag() == gimli::DW_TAG_variable ||
            die.tag() == gimli::DW_TAG_formal_parameter ||
                die.tag() == gimli::DW_TAG_constant { // Check that it is a variable.

            if let Ok(Some(DebugStrRef(offset))) =  die.attr_value(gimli::DW_AT_name) { // Get the name of the variable.
                return self.dwarf.string(offset).unwrap().to_string().unwrap() == search;// Compare the name of the variable. 

            } else if let Ok(Some(offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
                match offset {
                    UnitRef(o) => {
                        if let Ok(ndie) = unit.entry(o) {
                            return self.check_var_name(unit, pc, &ndie, search);
                        }
                    },
                    _ => {
                        println!("{:?}", offset);
                        unimplemented!();
                    },
                };
            }
        }
        return false;
    }


    fn get_var_type(&mut self,
                      unit:     &Unit<R>,
                      pc:       u32,
                    die: &DebuggingInformationEntry<R>
                    ) -> Option<DebuggerType>
    {
        if let Ok(Some(tattr)) =  die.attr_value(gimli::DW_AT_type) {
            return match self.parse_type_attr(unit, pc, tattr) {
                Ok(t) => Some(t),
                Err(_) => None,
            };
        } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match die_offset {
                UnitRef(offset) => {
                    if let Ok(ndie) = unit.entry(offset) {
                        return self.get_var_type(unit, pc, &ndie);
                    }
                },
                _ => {
                    println!("{:?}", die_offset);
                    unimplemented!();
                },
            };
        }
        return None;
    }


    fn eval_location(&mut self,
                      unit:     &Unit<R>,
                      pc:       u32,
                     die:               &DebuggingInformationEntry<R>,
                     dtype:             &DebuggerType,
                     frame_base:    Option<u64>
                     ) -> Result<Option<DebuggerValue<R>>> 
    {
        //println!("{:?}", die.attr_value(gimli::DW_AT_location));
        match die.attr_value(gimli::DW_AT_location)? {
            Some(Exprloc(expr)) => {
                self.print_die(&die)?;
                let value = self.evaluate(unit, pc, expr, frame_base, Some(dtype))?;
                println!("\n");

                return Ok(Some(value));
            },
            Some(LocationListsRef(offset)) => {
                self.print_die(&die)?;
                let mut locations = self.dwarf.locations(unit, offset)?;
                while let Some(llent) = locations.next()? {
                    //let value = self.evaluate(unit, llent.data, frame_base, Some(&dtype)).unwrap();
                    //println!("\n");
                    if in_range(pc, &llent.range) {
                        let value = self.evaluate(unit, pc, llent.data, frame_base, Some(dtype))?;
                        println!("\n");

                        return Ok(Some(value));
                    }
                }

                return Ok(Some(DebuggerValue::OutOfRange));
            },
            None => return Err(anyhow!("Expected dwarf location informaiton")),//unimplemented!(), //return Err(Error::Io), // TODO: Better error
            Some(v) => {
                println!("{:?}", v);
                unimplemented!();
            },
        }
    }
    

    pub fn check_frame_base(&mut self,
                            unit:     &Unit<R>,
                            pc:       u32,
                            die:        &DebuggingInformationEntry<'_, '_, R>,
                            frame_base: Option<u64>
                            ) -> Result<Option<u64>>
    {
        if let Some(val) = die.attr_value(gimli::DW_AT_frame_base)? {
            if let Some(expr) = val.exprloc_value() {
                return Ok(match self.evaluate(unit, pc, expr, frame_base, None) {
                    Ok(DebuggerValue::Value(Value::U64(v))) => Some(v),
                    Ok(DebuggerValue::Value(Value::U32(v))) => Some(v as u64),
                    Ok(v) => {
                        println!("{:?}", v);
                        unimplemented!()
                    },
                    Err(err) => panic!(err),
                });
            } else {
                return Ok(None);
            }
        } else {
            return Ok(frame_base);
        }
    }
}

