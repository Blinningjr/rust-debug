use super::{
    Debugger,
    types::{
        DebuggerType,
    },
};


use gimli::{
    AttributeValue,
    AttributeValue::{
        DebugInfoRef,
        UnitRef,
        Data1,
        Data2,
        Data4,
        Data8,
        Udata,
    },
    Reader,
    EntriesTreeNode,
};


use std::collections::HashMap;


impl<'a, R: Reader<Offset = usize>> Debugger<'a, R> {
    pub fn parse_type_attr(&mut self,
                           attr_value: AttributeValue<R>
                           ) -> gimli::Result<DebuggerType>
    {
        unimplemented!();
//        match attr_value {
//            UnitRef(offset) => {
//                let mut tree = self.unit.entries_tree(Some(offset))?;
//                let root = tree.root()?;
//                self.print_tree(root);
//                return Err(gimli::Error::Io);
//                return match root.entry().tag() { 
//                    gimli::DW_TAG_structure_type    => self.parse_structure_type(root),
//                    gimli::DW_TAG_base_type         => Ok(DebuggerType::BaseType(self.parse_base_type(root)?)),
//                    gimli::DW_TAG_union_type        => self.parse_union_type(root),
//                    gimli::DW_TAG_array_type        => self.parse_array_type(root),
//                    gimli::DW_TAG_enumeration_type  => self.parse_enumeration_type(root),
//                    gimli::DW_TAG_pointer_type      => self.parse_pointer_type(root),
//                    _ => {
//                        println!("Start of type tree");
//                        self.print_tree(root);
//                        unimplemented!(); //TODO: Add parser if this is reached.
//                    },
//                };
//            },
//            DebugInfoRef(di_offset) => {
//                let res = self.debug_info_offset_type(di_offset).ok_or_else(|| gimli::Error::Io)?;
//                println!("{:?}", res);
//                return Ok(res);
//            },
//            _ => {
//                println!("{:?}", attr_value);
//                unimplemented!();
//            },
//        };
    }


//    fn parse_structure_type(&mut self,
//                            node: EntriesTreeNode<R>
//                            ) -> gimli::Result<DebuggerType>
//    {
//        let die         = node.entry();
//        let name        = self.name_attribute(&die).unwrap();
//        let byte_size   = self.byte_size_attribute(&die).unwrap();
//        let alignment   = self.alignment_attribute(&die).unwrap();
//
//        let mut members: Vec<Member>    = Vec::new();
//        let mut children                = node.children();
//
//        while let Some(child) = children.next()? { 
//            match child.entry().tag() {
//                gimli::DW_TAG_variant_part => {
//                    let (index_type, variants) = self.parse_variant_part(child)?;
////                    continue;
//                    return Ok(DebuggerType::Enum(Enum {
//                        name:       name,
//                        byte_size:  byte_size,
//                        alignment:  alignment,
//                        index_type: index_type,
//                        variants:   variants,
//                    }));
//                },
//                gimli::DW_TAG_member => {
//                    let member = self.parse_member(child)?;
//                    members.push(member);
//                },
//                gimli::DW_TAG_template_type_parameter   => continue, //TODO
//                gimli::DW_TAG_subprogram                => continue, //TODO
//                gimli::DW_TAG_structure_type            => continue, //TODO
//                _ => {
//                    println!("Type tree starts here");
//                    self.print_tree(child);
//                    unimplemented!();
//                },
//            };
//        }
//       
//        return Ok(DebuggerType::Struct(Struct {
//            name:       name,
//            byte_size:  byte_size,
//            alignment:  alignment,
//            members:    members,
//        })); 
//    }
//
//
//    fn parse_variant_part(&mut self,
//                          node: EntriesTreeNode<R>
//                          ) -> gimli::Result<(ArtificialMember, HashMap<u64, Member>)>
//    {
//        let mut enum_index_type: Option<ArtificialMember>   = None;
//        let mut variants: HashMap<u64, Member>              = HashMap::new();
//
//        let mut children = node.children();
//        while let Some(child) = children.next()? {
//            match child.entry().tag() {
//                gimli::DW_TAG_variant => {
//                    let (id, val) = self.parse_variant(child)?;
//                    variants.insert(id, val);
//                },
//                gimli::DW_TAG_member => {
//                    if enum_index_type != None {
//                        panic!("Enum index type should not be set");
//                    }
//                    enum_index_type = Some(self.parse_artificial_member(child)?);
//                },
//                _ => (),
//            };
//        }
//
//        if let Some(index) = enum_index_type {
//            return Ok((index, variants)); 
//        }
//        panic!("Enum index type to have a value");
//    }
//
//
//    fn parse_variant(&mut self,
//                     node: EntriesTreeNode<R>
//                     ) -> gimli::Result<(u64, Member)>
//    {
//        let enum_index: u64 = match node.entry().attr_value(gimli::DW_AT_discr_value)? {
//            Some(Data1(val)) => val as u64,
//            Some(Data2(val)) => val as u64,
//            Some(Data4(val)) => val as u64,
//            Some(Data8(val)) => val,
//            Some(Udata(val)) => val,
//            _ => unimplemented!(),
//        };
//
//        let mut children = node.children();
//        while let Some(child) = children.next()? { // TODO: Can this node have more children?
//            match child.entry().tag() {
//                gimli::DW_TAG_member => {
//                    let member = self.parse_member(child)?;
//                    return Ok((enum_index, member));
//                },
//                _ => unimplemented!(),
//            };
//        }
//        panic!("Error: Expected one member");
//    }
//
//
//    fn parse_member(&mut self,
//                    node: EntriesTreeNode<R>
//                    ) -> gimli::Result<Member>
//    {
//        let die                     = node.entry();
//        let name                    = self.name_attribute(&die).unwrap();
//        let r#type                  = self.type_attribute(&die).unwrap();
//        let alignment               = self.alignment_attribute(&die).unwrap();
//        let data_member_location    = self.data_member_location_attribute(&die).unwrap();
//
//        return Ok(Member {
//            name:                   name,
//            r#type:                 Box::new(r#type),
//            alignment:              alignment,
//            data_member_location:   data_member_location,
//        });
//    }
//
//
//    fn parse_artificial_member(&mut self,
//                               node: EntriesTreeNode<R>
//                               ) -> gimli::Result<ArtificialMember>
//    {
//        let die                     = node.entry();
//        let r#type                  = self.type_attribute(&die).unwrap();
//        let alignment               = self.alignment_attribute(&die).unwrap();
//        let data_member_location    = self.data_member_location_attribute(&die).unwrap();
//
//        return Ok(ArtificialMember {
//            r#type:                 Box::new(r#type),
//            alignment:              alignment,
//            data_member_location:   data_member_location,
//        });
//    }
//
//
//    fn parse_base_type(&mut self,
//                       node: EntriesTreeNode<R>
//                       ) -> gimli::Result<BaseType>
//    {
//        let die         = node.entry();
//        let name        = self.name_attribute(&die).unwrap();
//        let encoding    = self.encoding_attribute(&die).unwrap(); 
//        let byte_size   = self.byte_size_attribute(&die).unwrap();
//
//        return Ok(BaseType {
//            name:       name,
//            encoding:   encoding,
//            byte_size:  byte_size,
//        });
//    }
//
//    fn parse_template_parameter_type(&mut self,
//                                     node: EntriesTreeNode<R>
//                                     ) -> gimli::Result<TemplateParameter>
//    {
//        let die     = node.entry();
//        let name    = self.name_attribute(&die).unwrap();
//        let r#type  = self.type_attribute(&die).unwrap();
//
//        return Ok(TemplateParameter {
//            name:   name,
//            r#type: Box::new(r#type),
//        });
//    }
//
//    fn parse_union_type(&mut self,
//                        node: EntriesTreeNode<R>
//                        ) -> gimli::Result<DebuggerType>
//    {
//        let die         = node.entry();
//        let name        = self.name_attribute(&die).unwrap();
//        let byte_size   = self.byte_size_attribute(&die).unwrap();
//        let alignment   = self.alignment_attribute(&die).unwrap();
//
//        let mut members: Vec<Member>            = Vec::new();
//        let mut tparams: Vec<TemplateParameter> = Vec::new();
//
//        let mut children = node.children();
//        while let Some(child) = children.next()? { // TODO: parse members and template type parameters
//            match child.entry().tag() {
//                gimli::DW_TAG_template_type_parameter => {
//                    let tpara = self.parse_template_parameter_type(child)?;
//                    tparams.push(tpara);
//                },
//                gimli::DW_TAG_member => {
//                    let member = self.parse_member(child)?;
//                    members.push(member);
//                },
//                _ => unimplemented!(),
//            };
//        }
//        
//        return Ok(DebuggerType::Union(UnionType {
//            name:       name,
//            byte_size:  byte_size,
//            alignment:  alignment,
//            members:    members,
//            tparams:    tparams,
//        })); 
//        
//    }
//
//    fn parse_array_type(&mut self,
//                        node: EntriesTreeNode<R>
//                        ) -> gimli::Result<DebuggerType>
//    {
//        let die     = node.entry();
//        let r#type  = self.type_attribute(&die).unwrap();
//
//        let mut children = node.children();
//        if let Some(child) = children.next()? { 
//            match child.entry().tag() {
//                gimli::DW_TAG_subrange_type => {
//                    let subrange = self.parse_subrange_type(child)?;
//                    return Ok(DebuggerType::Array(ArrayType {
//                        r#type: Box::new(r#type),
//                        range: subrange,
//                    }));
//                },
//                _ => unimplemented!(), //TODO: Implement if reached
//            };
//        }
//        unimplemented!(); //TODO: Implement if reached
//    }
//
//    fn parse_subrange_type(&mut self,
//                           node: EntriesTreeNode<R>
//                           ) -> gimli::Result<SubRangeType>
//    {
//        let die         = node.entry();
//        let r#type      = self.type_attribute(&die).unwrap(); 
//        let lower_bound = self.lower_bound_attribute(&die).unwrap();
//        
//        let count = match die.attr_value(gimli::DW_AT_count)? {
//            Some(attr) => match attr {
//                Data1(val) => val as u64,
//                Data2(val) => val as u64,
//                Data4(val) => val as u64,
//                Data8(val) => val,
//                Udata(val) => val,
//                _ => unimplemented!(),
//            },
//            _ => panic!("expected lower bound"),
//        }; 
//
//        return Ok(SubRangeType {
//            r#type:         Box::new(r#type),
//            lower_bound:    lower_bound,
//            count:          count,
//        });
//    }
//
//    fn parse_enumeration_type(&mut self,
//                              node: EntriesTreeNode<R>
//                              ) -> gimli::Result<DebuggerType>
//    {
//        let die         = node.entry();
//        let name        = self.name_attribute(&die).unwrap();
//        let byte_size   = self.byte_size_attribute(&die).unwrap();
//        let alignment   = self.alignment_attribute(&die).unwrap();
//        let r#type      = self.type_attribute(&die).unwrap();
//        let enum_class  = self.enum_class_attribute(&die).unwrap(); 
//
//        let mut enumerators = Vec::new(); 
//        let mut children    = node.children();
//        while let Some(child) = children.next()? { 
//            match child.entry().tag() {
//                gimli::DW_TAG_enumerator => {
//                    let enumerator = self.parse_enumerator_type(child)?;
//                    enumerators.push(enumerator);
//                },
//                _ => unimplemented!(),
//            };
//        }
//
//        return Ok(DebuggerType::EnumerationType(EnumerationType {
//            name:           name,
//            enum_class:     enum_class,
//            r#type:         Box::new(r#type),
//            byte_size:      byte_size,
//            alignment:      alignment,
//            enumerators:    enumerators,
//        }));
//    }
//
//
//    fn parse_enumerator_type(&mut self,
//                              node: EntriesTreeNode<R>
//                              ) -> gimli::Result<Enumerator>
//    {
//        let die         = node.entry();
//        let name        = self.name_attribute(&die).unwrap();
//        let const_value = self.const_value_attribute(&die).unwrap();
//
//        return Ok(Enumerator{
//            name:           name,
//            const_value:    const_value,
//        });
//    }
//
//
//    fn parse_pointer_type(&mut self,
//                          node: EntriesTreeNode<R>
//                          ) -> gimli::Result<DebuggerType>
//    {
//        let die             = node.entry();
//        let name            = self.name_attribute(&die).unwrap();
//        let address_class   = self.address_class_attribute(&die).unwrap();
//        let r#type          = self.type_attribute(&die).unwrap();
//
//        return Ok(DebuggerType::Pointer(PointerType {
//            name:           name,
//            r#type:         Box::new(r#type),
//            address_class:  address_class,
//        }));
//    }
//
//
//
//    fn debug_info_offset_type(&mut self, // TODO
//                              offset: gimli::DebugInfoOffset,
//                              ) -> Option<DebuggerType>
//    {
//        let offset = gimli::UnitSectionOffset::DebugInfoOffset(offset);
//        let mut iter = self.dwarf.debug_info.units();
//        while  let Ok(Some(header)) = iter.next() {
//            let unit = self.dwarf.unit(header).unwrap();
//            if let Some(offset) = offset.to_unit_offset(&unit) {
//                let mut tree = unit.entries_tree(Some(offset)).ok()?;
//                let root= tree.root().unwrap(); 
//                let die = root.entry();
//                return Some(DebuggerType::BaseType(self.parse_base_type(root).ok()?));
//            }
//        }
//        None
//    }
}

