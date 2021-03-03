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
        Exprloc,
        LocationListsRef,
        UnitRef,
    },
    Reader,
    EntriesTreeNode,
    Value,
    Error,
};


pub struct Debugger<'a, R: Reader<Offset = usize>> {
    core:   Core<'a>,
    dwarf:  Dwarf<R>,
    unit:   &'a Unit<R>,
    pc:     u32,
}


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn new(core:    Core<'a>,
               dwarf:   Dwarf<R>,
               unit:    &'a Unit<R>,
               pc:      u32
               ) -> Debugger<'a, R> {
        Debugger{
            core:   core,
            dwarf:  dwarf,
            unit:   unit,
            pc:     pc,
        }
    }


    pub fn find_variable(&mut self,
                         search: &str
                         ) -> gimli::Result<DebuggerValue<R>>
    {
        let mut tree    = self.unit.entries_tree(None)?;
        let root        = tree.root()?;

//        self.print_tree(root)?;
//        unimplemented!();

        return match self.process_tree(root, None, search)? {
            Some(val)   => Ok(val),
            None        => Err(Error::Io), // TODO: Change to a better error.
        };
    }


    pub fn process_tree(&mut self, 
                        node:           EntriesTreeNode<R>,
                        mut frame_base: Option<u64>,
                        search:         &str
                        ) -> gimli::Result<Option<DebuggerValue<R>>>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, &self.unit, die, self.pc) {
            Some(false) => return Ok(None),
            _ => (),
        };

        //frame_base = self.check_frame_base(&die, frame_base)?;

        // Check for the searched vairable.
        if self.check_var_name(&die, search) {
            println!("\n");
            self.print_die(&die)?;
            let dtype = self.get_var_type(&die).unwrap();
            println!("{:#?}", dtype);
            //match self.eval_location(&die, &dtype, frame_base) {
            //    Ok(v) => return Ok(v),
            //    Err(_) => (),
            //};
        }
        
        //self.print_die(&die)?;
//        if let Some(dtype) = self.get_var_type(&die) {
////            println!("{:#?}", dtype);
//            //self.print_die(&die)?;
//            //self.eval_location(&die, &dtype, frame_base);
//        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            if let Some(result) = self.process_tree(child, frame_base, search)? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }


    fn check_var_name(&mut self,
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
                        if let Ok(ndie) = self.unit.entry(o) {
                            return self.check_var_name(&ndie, search);
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
                    die: &DebuggingInformationEntry<R>
                    ) -> Option<DebuggerType>
    {
        if let Ok(Some(tattr)) =  die.attr_value(gimli::DW_AT_type) {
            return match self.parse_type_attr(tattr) {
                Ok(t) => Some(t),
                Err(_) => None,
            };
        } else if let Ok(Some(die_offset)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            match die_offset {
                UnitRef(offset) => {
                    if let Ok(ndie) = self.unit.entry(offset) {
                        return self.get_var_type(&ndie);
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
                     die:               &DebuggingInformationEntry<R>,
                     dtype:             &DebuggerType,
                     mut frame_base:    Option<u64>
                     ) -> gimli::Result<Option<DebuggerValue<R>>> 
    {
        //println!("{:?}", die.attr_value(gimli::DW_AT_location));
        match die.attr_value(gimli::DW_AT_location)? {
            Some(Exprloc(expr)) => {
                self.print_die(&die)?;
                let value = match self.evaluate(self.unit, expr, frame_base, Some(dtype)) {
                    Ok(val) => val,
                    Err(_) => return Err(Error::Io), // TODO
                };
                println!("\n");

                return Ok(Some(value));
            },
            Some(LocationListsRef(offset)) => {
                self.print_die(&die)?;
                let mut locations = self.dwarf.locations(self.unit, offset)?;
                while let Some(llent) = locations.next()? {
                    //let value = self.evaluate(self.unit, llent.data, frame_base, Some(&dtype)).unwrap();
                    //println!("\n");
                    if in_range(self.pc, &llent.range) {
                        let value = self.evaluate(self.unit, llent.data, frame_base, Some(dtype)).unwrap();
                        println!("\n");

                        return Ok(Some(value));
                    }
                }
                panic!("Location Out Of Range");
            },
            None => return Err(Error::Io),//unimplemented!(), //return Err(Error::Io),
            Some(v) => {
                println!("{:?}", v);
                unimplemented!();
            },
        }
    }
    

    pub fn check_frame_base(&mut self,
                            die:        &DebuggingInformationEntry<'_, '_, R>,
                            frame_base: Option<u64>
                            ) -> gimli::Result<Option<u64>>
    {
        if let Some(val) = die.attr_value(gimli::DW_AT_frame_base)? {
            if let Some(expr) = val.exprloc_value() {
                return Ok(match self.evaluate(&self.unit, expr, frame_base, None) {
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

