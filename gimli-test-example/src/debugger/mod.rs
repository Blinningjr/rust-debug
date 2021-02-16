pub mod utils;
pub mod print_dwarf;
pub mod evaluate;
pub mod type_parser;
pub mod type_value;


use utils::{
    die_in_range,
    in_range,
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
        Exprloc,
        LocationListsRef,
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
//        self.print_tree(root);
//        unimplemented!();
        return match self.process_tree(root, None, search)? {
            Some(val) => Ok(val),
            None => Err(Error::Io), // TODO: Change to a better error.
        };
    }


    pub fn process_tree(&mut self, 
            node: EntriesTreeNode<R>,
            mut frame_base: Option<u64>,
            search: &str
        ) -> gimli::Result<Option<DebuggerValue<R>>>
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

                    println!("\n");
                    self.print_die(&die);

                    return self.eval_location(&die, frame_base);
                }
            }
        }

        self.eval_location(&die, frame_base);

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            if let Some(result) = self.process_tree(child, frame_base, search)? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }


    fn eval_location(&mut self,
                     die: &DebuggingInformationEntry<R>,
                     mut frame_base: Option<u64>
                     ) -> gimli::Result<Option<DebuggerValue<R>>> 
    {
        if let Some(tattr) =  die.attr_value(gimli::DW_AT_type)? { 
            match die.attr_value(gimli::DW_AT_location)? {
                Some(Exprloc(expr)) => {

                    let dtype = self.parse_type_attr(tattr).unwrap();
                    let value = match self.evaluate(self.unit, expr, frame_base, Some(&dtype)) {
                        Ok(val) => val,
                        Err(_) => return Err(Error::Io), // TODO
                    };
                    println!("\n");

                    return Ok(Some(value));
                },
                Some(LocationListsRef(offset)) => {
                    let mut locations = self.dwarf.locations(self.unit, offset)?;
                    while let Some(llent) = locations.next()? {
                        if in_range(self.pc, &llent.range) {
                            let dtype = self.parse_type_attr(tattr).unwrap();
                            let value = self.evaluate(self.unit, llent.data, frame_base, Some(&dtype)).unwrap();
                            println!("\n");

                            return Ok(Some(value));
                        }
                    }
                    return Err(Error::Io);
                },
                None => return Err(Error::Io),
                Some(v) => {
                    println!("{:?}", v);
                    unimplemented!();
                },
            }
        } else {
            match die.attr_value(gimli::DW_AT_location)? {
                Some(Exprloc(expr)) => {

                    let value = self.evaluate(self.unit, expr, frame_base, None).unwrap();
                    println!("\n");

                    return Ok(Some(value));
                },
                Some(LocationListsRef(offset)) => {
                    let mut locations = self.dwarf.locations(self.unit, offset)?;
                    while let Some(llent) = locations.next()? {
                        if in_range(self.pc, &llent.range) {
                            let value = self.evaluate(self.unit, llent.data, frame_base, None).unwrap();
                            println!("\n");

                            return Ok(Some(value));
                        }
                    }
                    return Err(Error::Io);
                },
                None => return Err(Error::Io),
                Some(v) => {
                    println!("{:?}", v);
                    unimplemented!();
                },
            }
        }
        return Err(Error::Io); //TODO
    }
    


    pub fn check_frame_base(&mut self,
                            die: &DebuggingInformationEntry<'_, '_, R>,
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



