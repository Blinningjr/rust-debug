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
        Sdata,
        Encoding,
        Flag,
        AddressClass,
    },
    Reader,
    EntriesTreeNode,
    DwAte,
    DwAddr,
    Section,
};

use std::collections::HashMap;

pub trait TypeInfo {
    fn byte_size(&self) -> u64;
    fn alignment(&self) -> Option<u64>;
}

#[derive(Debug, PartialEq)]
pub enum DebuggerType {
    Enum(Enum),
    EnumerationType(EnumerationType),
    Struct(Struct),
    BaseType(BaseType),
    Union(UnionType),
    Array(ArrayType),
    Pointer(PointerType),
//    TemplateParameter(TemplateParameter),
    Non,
}
impl TypeInfo for DebuggerType {
    fn byte_size(&self) -> u64 {
        match self {
            DebuggerType::Enum(e) => e.byte_size(),
            DebuggerType::EnumerationType(et) => et.byte_size(),
            DebuggerType::Struct(s) => s.byte_size(),
            DebuggerType::BaseType(bt) => bt.byte_size(),
            DebuggerType::Union(ut) => ut.byte_size(),
            DebuggerType::Array(at) => at.byte_size(),
            DebuggerType::Pointer(pt) => pt.byte_size(),
            DebuggerType::Non => 0,
        }
    }
    fn alignment(&self) -> Option<u64>{
        match self {
            DebuggerType::Enum(e) => e.alignment(),
            DebuggerType::EnumerationType(et) => et.alignment(),
            DebuggerType::Struct(s) => s.alignment(),
            DebuggerType::BaseType(bt) => bt.alignment(),
            DebuggerType::Union(ut) => ut.alignment(),
            DebuggerType::Array(at) => at.alignment(),
            DebuggerType::Pointer(pt) => pt.alignment(),
            DebuggerType::Non => None,
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct BaseType {
    pub name: String,
    pub encoding: DwAte,
    pub byte_size: u64,
}

impl TypeInfo for BaseType {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        None
    }
}


#[derive(Debug, PartialEq)]
pub struct Struct {
    pub name: String,
    pub byte_size: u64,
    pub alignment: u64,
    pub members: Vec<Member>,
}

impl TypeInfo for Struct {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
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
impl TypeInfo for Enum {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}

#[derive(Debug, PartialEq)]
pub struct EnumerationType {
    pub name: String,
    pub enum_class: bool,
    pub r#type: Box<DebuggerType>,
    pub byte_size: u64,
    pub alignment: u64, 
    pub enumerators: Vec<Enumerator>,
}
impl TypeInfo for EnumerationType {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}

#[derive(Debug, PartialEq)]
pub struct Enumerator {
    pub name: String,
    pub const_value: u64,
}
impl TypeInfo for Enumerator {
    fn byte_size(&self) -> u64 {
        1
    }
    fn alignment(&self) -> Option<u64>{
        None
    }
}


#[derive(Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub r#type: Box<DebuggerType>,
    pub alignment: u64,
    pub data_member_location: u64,
}
impl TypeInfo for Member {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}


#[derive(Debug, PartialEq)]
pub struct ArtificialMember {
    pub r#type: Box<DebuggerType>,
    pub alignment: u64,
    pub data_member_location: u64,
}
impl TypeInfo for ArtificialMember {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}


#[derive(Debug, PartialEq)]
pub struct TemplateParameter {
    pub name: String,
    pub r#type: Box<DebuggerType>,
}
impl TypeInfo for TemplateParameter {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct UnionType {
    pub name: String,
    pub byte_size: u64,
    pub alignment: u64,
    pub members: Vec<Member>,
    pub tparams: Vec<TemplateParameter>,
}
impl TypeInfo for UnionType {
    fn byte_size(&self) -> u64 {
        self.byte_size
    }
    fn alignment(&self) -> Option<u64>{
        Some(self.alignment)
    }
}


#[derive(Debug, PartialEq)]
pub struct ArrayType {
    pub r#type: Box<DebuggerType>,
    pub range: SubRangeType,
}
impl TypeInfo for ArrayType {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct SubRangeType {
    pub r#type: Box<DebuggerType>,
    pub lower_bound: i64,
    pub count: u64,
}
impl TypeInfo for SubRangeType {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
    }
}


#[derive(Debug, PartialEq)]
pub struct PointerType {
    pub name: String,
    pub r#type: Box<DebuggerType>,
    pub address_class: DwAddr,
}
impl TypeInfo for PointerType {
    fn byte_size(&self) -> u64 {
        self.r#type.byte_size()
    }
    fn alignment(&self) -> Option<u64>{
        self.r#type.alignment()
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
                return match root.entry().tag() { 
                    gimli::DW_TAG_structure_type => self.parse_structure_type(root),
                    gimli::DW_TAG_base_type => Ok(DebuggerType::BaseType(self.parse_base_type(root)?)),
//                    gimli::DW_TAG_template_type_parameter => Ok(DebuggerType::TemplateParameter(self.parse_template_parameter_type(root)?)),
                    gimli::DW_TAG_union_type => self.parse_union_type(root),
                    gimli::DW_TAG_array_type => self.parse_array_type(root),
                    gimli::DW_TAG_enumeration_type => self.parse_enumeration_type(root),
                    gimli::DW_TAG_pointer_type => self.parse_pointer_type(root),
                    _ => {
                        println!("Start of type tree");
                        self.print_tree(root);
                        unimplemented!(); //TODO: Add parser if this is reached.
                    },
                };
            },
            DebugInfoRef(di_offset) => {
                let res = self.debug_info_offset_type(di_offset).ok_or_else(|| gimli::Error::Io)?;
                println!("{:?}", res);
                return Ok(res);
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
        let die = node.entry();
        let name: String = match die.attr_value(gimli::DW_AT_name)? {
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
        while let Some(child) = children.next()? { 
            match child.entry().tag() {
                gimli::DW_TAG_variant_part => {
                    let (index_type, variants) = self.parse_variant_part(child)?;
//                    continue;
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
                gimli::DW_TAG_template_type_parameter => continue, //TODO
                gimli::DW_TAG_subprogram => continue, //TODO
                gimli::DW_TAG_structure_type => continue, //TODO
                _ => {
                    println!("Type tree starts here");
                    self.print_tree(child);
                    unimplemented!();
                },
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

    fn parse_template_parameter_type(&mut self,
                            node: EntriesTreeNode<R>
                            ) -> gimli::Result<TemplateParameter>
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
        return Ok(TemplateParameter {
            name: name,
            r#type: Box::new(r#type),
        });
    }

    fn parse_union_type(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<DebuggerType>
    {
        let die = node.entry();
        let name: String = match die.attr_value(gimli::DW_AT_name)? {
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
        let mut tparams: Vec<TemplateParameter> = Vec::new();

        let mut children = node.children();
        while let Some(child) = children.next()? { // TODO: parse members and template type parameters
            match child.entry().tag() {
                gimli::DW_TAG_template_type_parameter => {
                    let tpara = self.parse_template_parameter_type(child)?;
                    tparams.push(tpara);
                },
                gimli::DW_TAG_member => {
                    let member = self.parse_member(child)?;
                    members.push(member);
                },
                _ => unimplemented!(),
            };
        }
        
        return Ok(DebuggerType::Union(UnionType {
            name: name,
            byte_size: byte_size,
            alignment: alignment,
            members: members,
            tparams: tparams,
        })); 
        
    }

    fn parse_array_type(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<DebuggerType>
    {
        let die = node.entry();
        let r#type = match die.attr_value(gimli::DW_AT_type)? {
            Some(attr) => self.parse_type_attr(attr)?,
            _ => panic!("expected Type"),
        }; 
        let mut children = node.children();
        if let Some(child) = children.next()? { 
            match child.entry().tag() {
                gimli::DW_TAG_subrange_type => {
                    let subrange = self.parse_subrange_type(child)?;
                    return Ok(DebuggerType::Array(ArrayType {
                        r#type: Box::new(r#type),
                        range: subrange,
                    }));
                },
                _ => unimplemented!(), //TODO: Implement if reached
            };
        }
        unimplemented!(); //TODO: Implement if reached
    }

    fn parse_subrange_type(&mut self,
                           node: EntriesTreeNode<R>
                           ) -> gimli::Result<SubRangeType>
    {
        let die = node.entry();
        let r#type = match die.attr_value(gimli::DW_AT_type)? {
            Some(attr) => self.parse_type_attr(attr)?,
            _ => panic!("expected Type"),
        }; 
        
        let lower_bound = match die.attr_value(gimli::DW_AT_lower_bound)? {
            Some(attr) => match attr {
                Sdata(val) => val,
                _ => unimplemented!(),
            },
            _ => panic!("expected lower bound"),
        }; 
        
        let count = match die.attr_value(gimli::DW_AT_count)? {
            Some(attr) => match attr {
                Data1(val) => val as u64,
                Data2(val) => val as u64,
                Data4(val) => val as u64,
                Data8(val) => val,
                Udata(val) => val,
                _ => unimplemented!(),
            },
            _ => panic!("expected lower bound"),
        }; 

        return Ok(SubRangeType {
            r#type: Box::new(r#type),
            lower_bound: lower_bound,
            count: count,
        });
    }

    fn parse_enumeration_type(&mut self,
                              node: EntriesTreeNode<R>
                              ) -> gimli::Result<DebuggerType>
    {
        let die = node.entry();
        let name: String = match die.attr_value(gimli::DW_AT_name)? {
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
        let r#type = match die.attr_value(gimli::DW_AT_type)? {
            Some(attr) => self.parse_type_attr(attr)?,
            _ => panic!("expected Type"),
        };
        let enum_class = match die.attr_value(gimli::DW_AT_enum_class)? {
            Some(Flag(b)) => b,
            _ => panic!("expected enum class flag"),
        }; 

        let mut enumerators = Vec::new();
        
        let mut children = node.children();
        while let Some(child) = children.next()? { 
            match child.entry().tag() {
                gimli::DW_TAG_enumerator => {
                    let enumerator = self.parse_enumerator_type(child)?;
                    enumerators.push(enumerator);
                },
                _ => unimplemented!(),
            };
        }

        return Ok(DebuggerType::EnumerationType(EnumerationType {
            name: name,
            enum_class: enum_class,
            r#type: Box::new(r#type),
            byte_size: byte_size,
            alignment: alignment,
            enumerators: enumerators,
        }));
    }


    fn parse_enumerator_type(&mut self,
                              node: EntriesTreeNode<R>
                              ) -> gimli::Result<Enumerator>
    {
        let die = node.entry();
        let name: String = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => self.dwarf.string(offset)?.to_string()?.to_string(),
            _ => panic!("expected name"),
        };
        let const_value: u64 = match die.attr_value(gimli::DW_AT_const_value)? {
            Some(Udata(val)) => val,
            _ => panic!("expected Udata"),
        };

        return Ok(Enumerator{
            name: name,
            const_value: const_value,
        });
    }


    fn parse_pointer_type(&mut self,
                              node: EntriesTreeNode<R>
                              ) -> gimli::Result<DebuggerType>
    {
        let die = node.entry();
        let name: String = match die.attr_value(gimli::DW_AT_name)? {
            Some(DebugStrRef(offset)) => self.dwarf.string(offset)?.to_string()?.to_string(),
            _ => panic!("expected name"),
        };
        let address_class: DwAddr = match die.attr_value(gimli::DW_AT_address_class)? {
            Some(AddressClass(val)) => val,
            _ => panic!("expected Udata"),
        };
        let r#type = match die.attr_value(gimli::DW_AT_type)? {
            Some(attr) => self.parse_type_attr(attr)?,
            _ => panic!("expected Type"),
        };

        return Ok(DebuggerType::Pointer(PointerType {
            name: name,
            r#type: Box::new(r#type),
            address_class: address_class,
        }));
    }



    fn debug_info_offset_type( // TODO
        &mut self,
        offset: gimli::DebugInfoOffset,
    ) -> Option<DebuggerType>
    {
        let offset = gimli::UnitSectionOffset::DebugInfoOffset(offset);
        let mut iter = self.dwarf.debug_info.units();
        while  let Ok(Some(header)) = iter.next() {
            let unit = self.dwarf.unit(header).unwrap();
            if let Some(offset) = offset.to_unit_offset(&unit) {
                let mut tree = unit.entries_tree(Some(offset)).ok()?;
                let root= tree.root().unwrap(); 
                let die = root.entry();
                return Some(DebuggerType::BaseType(self.parse_base_type(root).ok()?));
            }
        }
        None
    }
}

