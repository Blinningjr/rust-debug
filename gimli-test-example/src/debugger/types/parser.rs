use super::{
    Debugger,
    types::{
        DebuggerType,
        BaseType,
        PointerType,
        ArrayType,
        ArrayDimension,
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
    },
    Reader,
    EntriesTreeNode,
};


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
//            gimli::DW_TAG_enumerator                => Ok(DebuggerType::Enumerator(self.parse_enumerator(node)?)),
            gimli::DW_TAG_string_type               => Ok(DebuggerType::StringType(self.parse_string_type(node)?)),
//            gimli::DW_TAG_subrange_type             => Ok(DebuggerType::SubrangeType(self.parse_subrange_type(node)?)),
            gimli::DW_TAG_generic_subrange          => Ok(DebuggerType::GenericSubrangeType(self.parse_generic_subrange_type(node)?)),
            gimli::DW_TAG_template_type_parameter   => Ok(DebuggerType::TemplateTypeParameter(self.parse_template_type_parameter(node)?)),
            gimli::DW_TAG_variant_part              => Ok(DebuggerType::VariantPart(self.parse_variant_part(node)?)),
//            gimli::DW_TAG_variant                   => Ok(DebuggerType::Variant(self.parse_variant(node)?)),
            gimli::DW_TAG_subroutine_type           => Ok(DebuggerType::SubroutineType(self.parse_subroutine_type(node)?)),
            gimli::DW_TAG_subprogram                => Ok(DebuggerType::Subprogram(self.parse_subprogram(node)?)),
            _ => {
                println!("Start of type tree");
                self.print_tree(node)?;
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
                //self.print_tree(root)?;
                //return Err(gimli::Error::Io);
                return self.parse_type_node(root);           
            },
            DebugInfoRef(di_offset) => {
                let res = self.debug_info_offset_type(di_offset).ok_or_else(|| gimli::Error::Io)?;
                //println!("{:?}", res);
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
        let alignment   = self.alignment_attribute(&die);
        let bit_size    = self.bit_size_attribute(&die);
        let byte_size   = self.byte_size_attribute(&die);
        let name        = self.name_attribute(&die);

        let mut parsed_children = Vec::new();
        let mut children        = node.children();

        while let Some(child) = children.next()? { 
            parsed_children.push(Box::new(self.parse_type_node(child)?));
        }
       
        return Ok(StructuredType {
            alignment:      alignment,
            bit_size:       bit_size,
            byte_size:      byte_size,
            children:       parsed_children,
            name:           name,
        }); 
    }


    fn parse_union_type(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<UnionType>
    {
        let die         = node.entry();
        let alignment   = self.alignment_attribute(&die);
        let bit_size    = self.bit_size_attribute(&die);
        let byte_size   = self.byte_size_attribute(&die);
        let name        = self.name_attribute(&die);

        let mut parsed_children = Vec::new();
        let mut children        = node.children();

        while let Some(child) = children.next()? { 
            parsed_children.push(Box::new(self.parse_type_node(child)?));
        }
       
        return Ok(UnionType {
            alignment:  alignment,
            bit_size:   bit_size,
            byte_size:  byte_size,
            children:   parsed_children,
            name:       name,
        }); 
    }


    fn parse_member_type(&mut self,
                         node: EntriesTreeNode<R>
                         ) -> gimli::Result<MemberType>
    {
        let die = node.entry();
        return Ok(MemberType {
            accessibility:          self.accessibility_attribute(&die),
            alignment:              self.alignment_attribute(&die),
            artificial:             self.artificial_attribute(&die),
            bit_size:               self.bit_size_attribute(&die),
            byte_size:              self.byte_size_attribute(&die),
            data_bit_offset:        self.data_bit_offset_attribute(&die),
            data_member_location:   self.data_member_location_attribute(&die),
            mutable:                self.mutable_attribute(&die),
            name:                   self.name_attribute(&die),
            r#type:                 Box::new(self.type_attribute(&die).unwrap()),
        });
    }


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
            alignment:          self.alignment_attribute(&die),
        });
    }


    /**
     * NOTE: Array type dies should have the attribute `type` which represents the type stored in
     * the array.
     * They should also have one or more children that are of the tags `DW_TAG_subrange_type`,
     * `DW_TAG_enumeration_type` or `DW_TAG_generic_subrange`.
     * The generic_subrange tag is special in that if it is present the attribute `rank` needs to
     * be present and the array type should only have one child.
     * 
     * Source: Dwarf 5 spec.
     */
    fn parse_array_type(&mut self,
                        node: EntriesTreeNode<R>
                        ) -> gimli::Result<ArrayType>
    { 
        let die             = node.entry(); 
        let alignment       = self.alignment_attribute(&die);
        let bit_size        = self.bit_size_attribute(&die);
        let byte_size       = self.byte_size_attribute(&die);
        let name            = self.name_attribute(&die);
        let r#type          = Box::new(self.type_attribute(&die).unwrap());
        let mut dimensions  = Vec::new();

        let mut children = node.children();
        if let Some(child) = children.next()? { 
            match child.entry().tag() {
                gimli::DW_TAG_subrange_type     => dimensions.push(ArrayDimension::SubrangeType(self.parse_subrange_type(child)?)),
                gimli::DW_TAG_enumeration_type  => dimensions.push(ArrayDimension::EnumerationType(self.parse_enumeration_type(child)?)),
                _ => {
                    println!("Start of type tree");
                    self.print_tree(child)?;
                    unimplemented!(); //TODO: Add parser for generic_subrange.
                },
            };
        }
        
        return Ok(ArrayType {
            alignment:  alignment,
            bit_size:   bit_size,
            byte_size:  byte_size,
            name:       name,
            r#type:     r#type,
            dimensions: dimensions,
        });
    }


    /**
     * NOTE: Should only have Enumerator die children according to Dwarf 5 spec, but it seams that
     * subprogram dies can also be children.
     */
    fn parse_enumeration_type(&mut self,
                              node: EntriesTreeNode<R>
                              ) -> gimli::Result<EnumerationType>
    {
        let die             = node.entry();
        let accessibility   = self.accessibility_attribute(&die);
        let alignment       = self.alignment_attribute(&die);
        let bit_size        = self.bit_size_attribute(&die);
        let byte_size       = self.byte_size_attribute(&die);
        //let data_location   = self.data_location_attribute(&die);
        let enum_class      = self.enum_class_attribute(&die);
        let name            = self.name_attribute(&die);
        let r#type          = self.type_attribute(&die);
        
        let mut enumerators = Vec::new(); 
        let mut methods     = Vec::new();
        let mut children    = node.children();

        while let Some(child) = children.next()? { 
            match child.entry().tag() {
                gimli::DW_TAG_enumerator    => enumerators.push(self.parse_enumerator(child)?),
                gimli::DW_TAG_subprogram    => methods.push(self.parse_subprogram(child)?),
                _ => {
                    println!("Start of type tree");
                    self.print_tree(child)?;
                    unimplemented!(); //TODO: Add parser for generic_subrange.
                },
            };
        }

        return Ok(EnumerationType {
            accessibility:  accessibility,
            alignment:      alignment,
            bit_size:       bit_size,
            byte_size:      byte_size,
            //data_location:  data_location,
            enum_class:     enum_class,
            enumerations:   enumerators,
            methods:        methods,
            name:           name,
            r#type:         Box::new(r#type),
        });
    }


    /**
     * NOTE: Enumerator dies should have a name and const_value attribute. The name is the name if
     * this enum variant and the const_value is like the enum id for this variant.
     *
     * Source: Dwarf 5 spec.
     */
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
            address_class:  self.address_class_attribute(&die),
            alignment:      self.alignment_attribute(&die),
            bit_size:       self.bit_size_attribute(&die),
            byte_size:      self.byte_size_attribute(&die),
            name:           self.name_attribute(&die),
            r#type:         Box::new(self.type_attribute(&die).unwrap()),
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
            accessibility:              self.accessibility_attribute(&die),
            alignment:                  self.alignment_attribute(&die),
            bit_size:                   self.bit_size_attribute(&die),
            byte_size:                  self.byte_size_attribute(&die),
            name:                       self.name_attribute(&die),
            string_length:              self.string_length_attribute(&die),
            string_length_bit_size:     self.string_length_bit_size_attribute(&die), 
            string_length_byte_size:    self.string_length_byte_size_attribute(&die),
        });
    }


    fn parse_subrange_type(&mut self,
                           node: EntriesTreeNode<R>
                           ) -> gimli::Result<SubrangeType>
    {
        let die = node.entry();
        return Ok(SubrangeType {
            accessibility:  self.accessibility_attribute(&die),
            alignment:      self.alignment_attribute(&die),
            bit_size:       self.bit_size_attribute(&die),
            byte_size:      self.byte_size_attribute(&die),
            count:          self.count_attribute(&die),
            lower_bound:    self.lower_bound_attribute(&die),
            name:           self.name_attribute(&die),
            r#type:         Box::new(self.type_attribute(&die)),
            upper_bound:    self.upper_bound_attribute(&die),
        });
    }
    

    fn parse_generic_subrange_type(&mut self,
                                   node: EntriesTreeNode<R>
                                   ) -> gimli::Result<GenericSubrangeType>
    {
        let die = node.entry();
        return Ok(GenericSubrangeType {
            accessibility:  self.accessibility_attribute(&die),
            alignment:      self.alignment_attribute(&die),
            bit_size:       self.bit_size_attribute(&die),
            byte_size:      self.byte_size_attribute(&die),
            count:          self.count_attribute(&die),
            lower_bound:    self.lower_bound_attribute(&die),
            name:           self.name_attribute(&die),
            r#type:         Box::new(self.type_attribute(&die)),
            upper_bound:    self.upper_bound_attribute(&die),
        });
    }


    fn parse_template_type_parameter(&mut self,
                                     node: EntriesTreeNode<R>
                                     ) -> gimli::Result<TemplateTypeParameter>
    {
        let die = node.entry();
        return Ok(TemplateTypeParameter {
//            default_value:  self.default_value_attribute(&die), // TODO
            name:           self.name_attribute(&die),
            r#type:         Box::new(self.type_attribute(&die).unwrap()),
        });
    }


    /**
     *  NOTE: Variant part dies should have ONE child that is a member die. This member die is
     *  the type of the value that decides which variant the structure is.
     *  The rest of the children should be variant dies that contain information about each
     *  variant.
     *
     * Source: Dwarf 5 spec.
     */
    fn parse_variant_part(&mut self,
                          node: EntriesTreeNode<R>
                          ) -> gimli::Result<VariantPart>
    {
        let die             = node.entry();
        let accessibility   = self.accessibility_attribute(&die);
//        let discr           = self.discr_attribute(&die);
        
        let mut member      = None;
        let mut variants    = Vec::new();
        let mut children    = node.children();

        while let Some(child) = children.next()? { 
            match child.entry().tag() {
                gimli::DW_TAG_member    => {
                    if member != None {
                        panic!("Expected only one member");
                    }
                    member = Some(self.parse_member_type(child)?);
                },
                gimli::DW_TAG_variant   => variants.push(self.parse_variant(child)?),
                _ => {
                    println!("Start of type tree");
                    self.print_tree(child)?;
                    unimplemented!(); //TODO: Add parser if this is reached.
                },
            };
        }

        return Ok(VariantPart {
            accessibility:  accessibility,
//            discr:          discr,
            member:         member,
            variants:       variants,
        });
    }


    /**
     * NOTE: Variant dies should contain some `discr` attribute that specifies which variant this
     * is. It should also have ONE child and that child should be a member die.
     *
     * Source: Dwarf 5 spec.
     */
    fn parse_variant(&mut self,
                     node: EntriesTreeNode<R>
                     ) -> gimli::Result<Variant>
    {
        let die             = node.entry();
        let accessibility   = self.accessibility_attribute(&die);
        let discr_value     = self.discr_value_attribute(&die);

        let mut member = None;
        let mut children        = node.children();

        let mut i = 0;
        while let Some(child) = children.next()? { 
            match child.entry().tag() {
                gimli::DW_TAG_member    => member = Some(self.parse_member_type(child)?),
                _ => {
                    println!("Start of type tree");
                    self.print_tree(child)?;
                    unimplemented!(); //TODO: Add parser if this is reached.
                },
            };
            i += 1;
        }

        if i != 1 {
            panic!("Expacted one member");
        }

        return Ok(Variant {
            accessibility:  accessibility,
            discr_value:    discr_value,
            member:         member.unwrap(),
        });
    }


    fn parse_subroutine_type(&mut self,
                             node: EntriesTreeNode<R>
                             )-> gimli::Result<SubroutineType>
    {
        let die = node.entry();
        return Ok(SubroutineType {
            accessibility:  self.accessibility_attribute(&die),
            address_class:  self.address_class_attribute(&die),
            alignment:      self.alignment_attribute(&die),
            name:           self.name_attribute(&die),
            linkage_name:   self.linkage_name_attribute(&die),
            r#type:         Box::new(self.type_attribute(&die)),
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

