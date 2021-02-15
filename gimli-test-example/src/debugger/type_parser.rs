use super::{
    Debugger,
    utils::{
        die_in_range,
    },
};


use gimli::{
    AttributeValue,
    AttributeValue::{
        DebugStrRef,
        DebugInfoRef,
        UnitRef,
        Data1,
        Data2,
        Data4,
        Data8,
        Udata,
        Encoding,
    },
    Reader,
    EntriesTreeNode,
    DwAte,
};

use std::collections::HashMap;

pub trait ByteSize {
    fn byte_size(&self) -> u64;
}

#[derive(Debug, PartialEq)]
pub enum DebuggerType {
    Enum(Enum),
    Struct(Struct),
    BaseType(BaseType),
    Non,
}

impl ByteSize for DebuggerType {
    fn byte_size(&self) -> u64 {
        match self {
            DebuggerType::Enum(e) => e.byte_size(),
            DebuggerType::Struct(s) => s.byte_size(),
            DebuggerType::BaseType(bt) => bt.byte_size(),
            DebuggerType::Non => 0,
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct BaseType {
    pub name: String,
    pub encoding: DwAte,
    pub byte_size: u64,
}

impl ByteSize for BaseType {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
}


#[derive(Debug, PartialEq)]
pub struct Struct {
    pub name: String,
    pub byte_size: u64,
    pub alignment: u64,
    pub members: Vec<Member>,
}

impl ByteSize for Struct {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
}


#[derive(Debug, PartialEq)]
pub struct Enum {
    pub name: String,
    pub byte_size: u64,
    pub alignment: u64,
    pub index_type: ArtificialMember,
    pub variants: HashMap<u64, Member>,
}

impl ByteSize for Enum {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
}


#[derive(Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub r#type: Box<DebuggerType>,
    pub alignment: u64,
    pub data_member_location: u64,
}

impl ByteSize for Member {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
}


#[derive(Debug, PartialEq)]
pub struct ArtificialMember {
    pub r#type: Box<DebuggerType>,
    pub alignment: u64,
    pub data_member_location: u64,
}

impl ByteSize for ArtificialMember {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
}


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn find_type(&mut self, search: &str) -> gimli::Result<()> {
        let mut tree = self.unit.entries_tree(None)?;
        let root = tree.root()?;
        self.process_tree_type(root, None, search)?;
        return Ok(());
    }


    pub fn process_tree_type(&mut self, 
            node: EntriesTreeNode<R>,
            mut frame_base: Option<u64>,
            search: &str
        ) -> gimli::Result<bool>
    {
        let die = node.entry();

        // Check if die in range
        match die_in_range(&self.dwarf, &self.unit, die, self.pc) {
            Some(false) => return Ok(false),
            _ => (),
        };

        frame_base = self.check_frame_base(&die, frame_base)?;

        // Check for the searched type.
        if let Some(DebugStrRef(offset)) =  die.attr_value(gimli::DW_AT_name)? { // Get the name of the variable.
            if self.dwarf.string(offset).unwrap().to_string().unwrap() == search { // Compare the name of the variable.
                self.print_tree(node);

                // Recursively process the children.
                //let mut i = 0;
                //let mut children = node.children();
                //while let Some(child) = children.next()? {
                //    if i == -1 {
                //        self.print_tree(child);
                //    }

                //    i += 1;
                //}

                return Ok(true);

            }
        }

        // Recursively process the children.
        let mut children = node.children();
        while let Some(child) = children.next()? {
            if self.process_tree_type(child, frame_base, search)? {
                return Ok(true);
            }
        }
        Ok(false)
    }


    pub fn parse_type_attr(&mut self,
                           attr_value: AttributeValue<R>
                           ) -> gimli::Result<DebuggerType>
    {
        match attr_value {
            UnitRef(offset) => {
                let mut tree = self.unit.entries_tree(Some(offset))?;
                let root = tree.root()?;
                return match root.entry().tag() { // TODO: Parse enum, struct and base types.
                    gimli::DW_TAG_structure_type => self.parse_structure_type(root),
                    gimli::DW_TAG_base_type => Ok(DebuggerType::BaseType(self.parse_base_type(root)?)),
                    _ => {
                        self.print_tree(root);
                        unimplemented!();
                    },
                };
            },
            DebugInfoRef(di_offset) => {
                println!("{:?}", self.dwarf.debug_info.header_from_offset(di_offset));
                return Ok(DebuggerType::Non);
                unimplemented!();
            },
            _ => {
                println!("{:?}", attr_value);
                unimplemented!();
            },
        };
    }


    fn parse_structure_type(&mut self,
                            node: EntriesTreeNode<R>
                            ) -> gimli::Result<DebuggerType>
    {
        let die = node.entry(); let name: String = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => self.dwarf.string(offset)?.to_string()?.to_string(),
            _ => panic!("expected name"),
        };
        let byte_size: u64 = match die.attr_value(gimli::DW_AT_byte_size)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };
        let alignment: u64 = match die.attr_value(gimli::DW_AT_alignment)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };

        let mut members: Vec<Member> = Vec::new();

        let mut children = node.children();
        while let Some(child) = children.next()? { // TODO: parse enum and struct.
            match child.entry().tag() {
                gimli::DW_TAG_variant_part => {
                    let (index_type, variants) = self.parse_variant_part(child)?;
                    return Ok(DebuggerType::Enum(Enum {
                        name: name,
                        byte_size: byte_size,
                        alignment: alignment,
                        index_type: index_type,
                        variants: variants,
                    }));
                },
                gimli::DW_TAG_member => {
                    let member = self.parse_member(child)?;
                    members.push(member);
                },
                _ => unimplemented!(),
            };
        }
        
        return Ok(DebuggerType::Struct(Struct {
            name: name,
            byte_size: byte_size,
            alignment: alignment,
            members: members,
        })); 
    }


    fn parse_variant_part(&mut self,
                          node: EntriesTreeNode<R>
                          ) -> gimli::Result<(ArtificialMember, HashMap<u64, Member>)>
    {
        let mut enum_index_type: Option<ArtificialMember> = None;
        let mut variants: HashMap<u64, Member> = HashMap::new();

        let mut children = node.children();
        while let Some(child) = children.next()? {
            match child.entry().tag() {
                gimli::DW_TAG_variant => {
                    let (id, val) = self.parse_variant(child)?;
                    variants.insert(id, val);
                },
                gimli::DW_TAG_member => {
                    if enum_index_type != None {
                        panic!("Enum index type should not be set");
                    }
                    enum_index_type = Some(self.parse_artificial_member(child)?);
                },
                _ => (),
            };
        }

        if let Some(index) = enum_index_type {
            return Ok((index, variants)); 
        }
        panic!("Enum index type to have a value");
    }


    fn parse_variant(&mut self,
                     node: EntriesTreeNode<R>
                     ) -> gimli::Result<(u64, Member)>
    {
        let enum_index: u64 = match node.entry().attr_value(gimli::DW_AT_discr_value)? {
            Some(Data1(val)) => val as u64,
            Some(Data2(val)) => val as u64,
            Some(Data4(val)) => val as u64,
            Some(Data8(val)) => val,
            Some(Udata(val)) => val,
            _ => unimplemented!(),
        };

        let mut children = node.children();
        while let Some(child) = children.next()? { // TODO: Can this node have more children?
            match child.entry().tag() {
                gimli::DW_TAG_member => {
                    let member = self.parse_member(child)?;
                    return Ok((enum_index, member));
                },
                _ => unimplemented!(),
            };
        }
        panic!("Error: Expected one member");
    }


    fn parse_member(&mut self,
                            node: EntriesTreeNode<R>
                            ) -> gimli::Result<Member>
    {
        let die = node.entry();
        let name: String = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => self.dwarf.string(offset)?.to_string()?.to_string(),
            _ => panic!("expected name"),
        };

        let r#type = match die.attr_value(gimli::DW_AT_type)? {
            Some(attr) => self.parse_type_attr(attr)?,
            _ => panic!("expected Type"),
        }; 

        let alignment: u64 = match die.attr_value(gimli::DW_AT_alignment)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };
        let data_member_location: u64 = match die.attr_value(gimli::DW_AT_data_member_location)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };
        return Ok(Member {
            name: name,
            r#type: Box::new(r#type),
            alignment: alignment,
            data_member_location: data_member_location,
        });
    }


    fn parse_artificial_member(&mut self,
                               node: EntriesTreeNode<R>
                               ) -> gimli::Result<ArtificialMember>
    {
        let die = node.entry();

        let r#type = match die.attr_value(gimli::DW_AT_type)? {
            Some(attr) => self.parse_type_attr(attr)?,
            _ => panic!("expected Type"),
        }; 

        let alignment: u64 = match die.attr_value(gimli::DW_AT_alignment)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };
        let data_member_location: u64 = match die.attr_value(gimli::DW_AT_data_member_location)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };
        return Ok(ArtificialMember {
            r#type: Box::new(r#type),
            alignment: alignment,
            data_member_location: data_member_location,
        });
    }


    fn parse_base_type(&mut self,
                      node: EntriesTreeNode<R>
                      ) -> gimli::Result<BaseType>
    {
        let die = node.entry();
        let name: String = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => self.dwarf.string(offset)?.to_string()?.to_string(),
            _ => panic!("expected name"),
        };
        
        let encoding: DwAte = match die.attr_value(gimli::DW_AT_encoding)? {
            Some(Encoding(val)) => val,
            _ => panic!("expected Udata"),
        };
        
        let byte_size: u64 = match die.attr_value(gimli::DW_AT_byte_size)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };

        return Ok(BaseType {
            name: name,
            encoding: encoding,
            byte_size: byte_size,
        });
    }
}

