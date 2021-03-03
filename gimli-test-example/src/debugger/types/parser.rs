use super::{
    Debugger,
    types::{
        DebuggerType,
        BaseType,
        PointerType,
        ArrayType,
        StructuredType,
        UnionType,
        EnumerationType,
        Enumerator,
        MemberType,
        StringType,
        SubrangeType,
        GenericSubrangeType,
        TemplateTypeParameter,
        VariantPart,
        Variant,
        SubroutineType,
        Subprogram,
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
    pub fn parse_type_node(&mut self,
                           node: EntriesTreeNode<R>
                           ) -> gimli::Result<DebuggerType>
    {
        return match node.entry().tag() { 
            gimli::DW_TAG_base_type                 => Ok(DebuggerType::BaseType(self.parse_base_type(node)?)),
            gimli::DW_TAG_pointer_type              => Ok(DebuggerType::PointerType(self.parse_pointer_type(node)?)),
            gimli::DW_TAG_array_type                => Ok(DebuggerType::ArrayType(self.parse_array_type(node)?)),
            gimli::DW_TAG_structure_type            => Ok(DebuggerType::StructuredType(self.parse_structure_type(node)?)),
            gimli::DW_TAG_union_type                => Ok(DebuggerType::UnionType(self.parse_union_type(node)?)),
            gimli::DW_TAG_member                    => Ok(DebuggerType::MemberType(self.parse_member_type(node)?)),
            gimli::DW_TAG_enumeration_type          => Ok(DebuggerType::EnumerationType(self.parse_enumeration_type(node)?)),
            gimli::DW_TAG_enumerator                => Ok(DebuggerType::Enumerator(self.parse_enumerator(node)?)),
            gimli::DW_TAG_string_type               => Ok(DebuggerType::StringType(self.parse_string_type(node)?)),
            gimli::DW_TAG_subrange_type             => Ok(DebuggerType::SubrangeType(self.parse_subrange_type(node)?)),
            gimli::DW_TAG_generic_subrange          => Ok(DebuggerType::GenericSubrangeType(self.parse_generic_subrange_type(node)?)),
            gimli::DW_TAG_template_type_parameter   => Ok(DebuggerType::TemplateTypeParameter(self.parse_template_type_parameter(node)?)),
            gimli::DW_TAG_variant_part              => Ok(DebuggerType::VariantPart(self.parse_variant_part(node)?)),
            gimli::DW_TAG_variant                   => Ok(DebuggerType::Variant(self.parse_variant(node)?)),
            gimli::DW_TAG_subroutine_type           => Ok(DebuggerType::SubroutineType(self.parse_subroutine_type(node)?)),
            gimli::DW_TAG_subprogram                => Ok(DebuggerType::Subprogram(self.parse_subprogram(node)?)),
            _ => {
                println!("Start of type tree");
                self.print_tree(node);
                unimplemented!(); //TODO: Add parser if this is reached.
            },
        };
    }


    pub fn parse_type_attr(&mut self,
                           attr_value: AttributeValue<R>
                           ) -> gimli::Result<DebuggerType>
    {
        match attr_value {
            UnitRef(offset) => {
                let mut tree = self.unit.entries_tree(Some(offset))?;
                let root = tree.root()?;
                //self.print_tree(root);
                //return Err(gimli::Error::Io);
                return self.parse_type_node(root);           
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
                            ) -> gimli::Result<StructuredType>
    {
        let die         = node.entry();
        let name        = self.name_attribute(&die);
        let byte_size   = self.byte_size_attribute(&die);
        let bit_size    = self.bit_size_attribute(&die);
        let alignment   = self.alignment_attribute(&die);

        let mut parsed_children = Vec::new();
        let mut children        = node.children();

        while let Some(child) = children.next()? { 
            parsed_children.push(Box::new(self.parse_type_node(child)?));
        }
       
        return Ok(StructuredType {
            name:       name,
            byte_size:  byte_size,
            bit_size:   bit_size,
            alignment:  alignment,
            children:   parsed_children,
        }); 
    }


    fn parse_union_type(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<UnionType>
    {
        let die         = node.entry();
        let name        = self.name_attribute(&die);
        let byte_size   = self.byte_size_attribute(&die);
        let bit_size    = self.bit_size_attribute(&die);
        let alignment   = self.alignment_attribute(&die);

        let mut parsed_children = Vec::new();
        let mut children        = node.children();

//        while let Some(child) = children.next()? { 
//            parsed_children.push(Box::new(self.parse_type_node(child)?));
//        }
       
        return Ok(UnionType {
            name:       name,
            byte_size:  byte_size,
            bit_size:   bit_size,
            alignment:  alignment,
            children:   parsed_children,
        }); 
    }


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


    fn parse_member_type(&mut self,
                         node: EntriesTreeNode<R>
                         ) -> gimli::Result<MemberType>
    {
        let die = node.entry();
        return Ok(MemberType {
            name:                   self.name_attribute(&die),
            r#type:                 Box::new(self.type_attribute(&die).unwrap()),
            accessibility:          self.accessibility_attribute(&die),
            mutable:                self.mutable_attribute(&die),
            data_member_location:   self.data_member_location_attribute(&die),
            data_bit_offset:        self.data_bit_offset_attribute(&die),
            byte_size:              self.byte_size_attribute(&die),
            bit_size:               self.bit_size_attribute(&die),
            alignment:              self.alignment_attribute(&die),
        });
    }


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


    fn parse_base_type(&mut self,
                       node: EntriesTreeNode<R>
                       ) -> gimli::Result<BaseType>
    {
        let die = node.entry();
        return Ok(BaseType {
            name:               self.name_attribute(&die),
            encoding:           self.encoding_attribute(&die).unwrap(),
            byte_size:          self.byte_size_attribute(&die),
            bit_size:           self.bit_size_attribute(&die),
            data_bit_offset:    self.data_bit_offset_attribute(&die),
        });
    }


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


    fn parse_array_type(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<ArrayType>
    { 
        let die                 = node.entry(); 
        let name                = self.name_attribute(&die);
        let r#type              = Box::new(self.type_attribute(&die).unwrap());
        let mut parsed_children = Vec::new();

        let mut children = node.children();
//        if let Some(child) = children.next()? { 
//            parsed_children.push(Box::new(self.parse_type_node(child)?));
//        }
        
        return Ok(ArrayType {
            name:           name,
            r#type:         r#type,
            children:       parsed_children,

            //ordering:       self.ordering_attribute(&die),
            //byte_stride:    self.byte_stride_attribute(&die),
            //bit_stride:     self.bit_stride_attribute(&die),
            //byte_size:      self.byte_size_attribute(&die),
            //bit_size:       self.bit_size_attribute(&die),
            //rank:           self.rank_attribute(&die),
            //allocated:      self.allocated_attribute(&die),
            //associated:     self.associated_attribute(&die),
            //data_location:  self.data_location_attribute(&die),
        });
    }


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


    fn parse_enumeration_type(&mut self,
                              node: EntriesTreeNode<R>
                              ) -> gimli::Result<EnumerationType>
    {
        let die         = node.entry();
        let name        = self.name_attribute(&die);
        let r#type      = self.type_attribute(&die);
        let byte_size   = self.byte_size_attribute(&die);
        let bit_size    = self.bit_size_attribute(&die);
        let alignment   = self.alignment_attribute(&die);
        let enum_class  = self.enum_class_attribute(&die);
        //let byte_stride = self.byte_stride_attribute(&die);
        //let bit_stride  = self.bit_stride_attribute(&die);
        
        let mut enumerators = Vec::new(); 
        let mut children    = node.children();

//        while let Some(child) = children.next()? { 
//            enumerators.push(Box::new(self.parse_type_node(child)?));
//        }

        return Ok(EnumerationType {
            name:           name,
            r#type:         Box::new(r#type),
            byte_size:      byte_size,
            bit_size:       bit_size,
            alignment:      alignment,
            enum_class:     enum_class,
            enumerations:   enumerators,

            //byte_stride:    byte_stride,
            //bit_stride:    bit_stride,
        });
    }


    fn parse_enumerator(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<Enumerator>
    {
        let die         = node.entry();
        let name        = self.name_attribute(&die).unwrap();
        let const_value = self.const_value_attribute(&die).unwrap();

        return Ok(Enumerator{
            name:           name,
            const_value:    const_value,
        });
    }


    fn parse_pointer_type(&mut self,
                          node: EntriesTreeNode<R>
                          ) -> gimli::Result<PointerType>
    {
        let die = node.entry();
        return Ok(PointerType {
            name:           self.name_attribute(&die),
            r#type:         Box::new(self.type_attribute(&die).unwrap()),
            address_class:  self.address_class_attribute(&die),
        });
    }


    fn debug_info_offset_type(&mut self, // TODO
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
                return Some(self.parse_type_node(root).ok()?);
            }
        }
        None
    }


    fn parse_string_type(&mut self,
                         node: EntriesTreeNode<R>
                         ) -> gimli::Result<StringType>
    {
        let die = node.entry();
        return Ok(StringType {
            name:                       self.name_attribute(&die),
            r#type:                     Box::new(self.type_attribute(&die)),
            byte_size:                  self.byte_size_attribute(&die),
            bit_size:                   self.bit_size_attribute(&die),
            alignment:                  self.alignment_attribute(&die),
            string_length:              self.string_length_attribute(&die),
            string_length_byte_size:    self.string_length_byte_size_attribute(&die),
            string_length_bit_size:     self.string_length_bit_size_attribute(&die), 
        });
    }


    fn parse_subrange_type(&mut self,
                           node: EntriesTreeNode<R>
                           ) -> gimli::Result<SubrangeType>
    {
        let die = node.entry();
        return Ok(SubrangeType {
            name:                       self.name_attribute(&die),
            r#type:                     Box::new(self.type_attribute(&die)),
            byte_size:                  self.byte_size_attribute(&die),
            bit_size:                   self.bit_size_attribute(&die),
            //threads_scaled:             self.threads_scaled_attribute(&die),
            lower_bound:                self.lower_bound_attribute(&die),
            upper_bound:                self.upper_bound_attribute(&die),
            count:                      self.count_attribute(&die),
            //byte_stride:                self.byte_stride_attribute(&die),
            //bit_stride:                 self.bit_stride_attribute(&die),
        });
    }
    

    fn parse_generic_subrange_type(&mut self,
                                   node: EntriesTreeNode<R>
                                   ) -> gimli::Result<GenericSubrangeType>
    {
        let die = node.entry();
        return Ok(GenericSubrangeType {
            name:                       self.name_attribute(&die),
            r#type:                     Box::new(self.type_attribute(&die)),
            byte_size:                  self.byte_size_attribute(&die),
            bit_size:                   self.bit_size_attribute(&die),
            //threads_scaled:             self.threads_scaled_attribute(&die),
            lower_bound:                self.lower_bound_attribute(&die),
            upper_bound:                self.upper_bound_attribute(&die),
            count:                      self.count_attribute(&die),
            //byte_stride:                self.byte_stride_attribute(&die),
            //bit_stride:                 self.bit_stride_attribute(&die),
        });
    }


    fn parse_template_type_parameter(&mut self,
                                     node: EntriesTreeNode<R>
                                     ) -> gimli::Result<TemplateTypeParameter>
    {
        let die = node.entry();
        return Ok(TemplateTypeParameter {
            name:   self.name_attribute(&die),
            r#type: Box::new(self.type_attribute(&die).unwrap()),
        });
    }


    fn parse_variant_part(&mut self,
                          node: EntriesTreeNode<R>
                          ) -> gimli::Result<VariantPart>
    {
        let die     = node.entry();
        let r#type  = Box::new(self.type_attribute(&die));

        let mut parsed_children = Vec::new();
        let mut children        = node.children();

//        while let Some(child) = children.next()? { 
//            parsed_children.push(Box::new(self.parse_type_node(child)?));
//        }

        return Ok(VariantPart {
            r#type:     r#type,
            children:   parsed_children,
        });
    }


    fn parse_variant(&mut self,
                     node: EntriesTreeNode<R>
                     ) -> gimli::Result<Variant>
    {
        let die         = node.entry();
        let discr_value  = self.discr_value_attribute(&die);

        let mut parsed_children = Vec::new();
        let mut children        = node.children();

//        while let Some(child) = children.next()? { 
//            parsed_children.push(Box::new(self.parse_type_node(child)?));
//        }

        return Ok(Variant {
            discr_value:    discr_value,
            children:       parsed_children,
        });
    }


    fn parse_subroutine_type(&mut self,
                             node: EntriesTreeNode<R>
                             )-> gimli::Result<SubroutineType>
    {
        let die = node.entry();
        return Ok(SubroutineType {
            name:               self.name_attribute(&die),
            linkage_name:       self.linkage_name_attribute(&die),
            r#type:             Box::new(self.type_attribute(&die)),
        });
    }


    fn parse_subprogram(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<Subprogram>
    {
        let die = node.entry();
        return Ok(Subprogram {
            name:               self.name_attribute(&die),
            linkage_name:       self.linkage_name_attribute(&die),
            //r#type:             Box::new(self.type_attribute(&die)),
        });
    }
}

