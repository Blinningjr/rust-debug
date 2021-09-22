use super::attributes;
use crate::call_stack::MemoryAccess;
use crate::registers::Registers;
use std::convert::TryInto;

use gimli::{DwAte, Location, Piece, Reader};

use anyhow::{anyhow, bail, Result};

use std::fmt;

#[derive(Debug, Clone)]
pub enum EvaluatorValue<R: Reader<Offset = usize>> {
    Value(BaseValue, ValueInformation),
    Bytes(R),

    Array(Box<ArrayValue<R>>),
    Struct(Box<StructValue<R>>),
    Enum(Box<EnumValue<R>>),
    Union(Box<UnionValue<R>>),
    Member(Box<MemberValue<R>>),
    Name(String),

    OutOfRange,   // NOTE: Variable does not have a value currently.
    OptimizedOut, // NOTE: Value is optimized out.
    ZeroSize,
}

impl<R: Reader<Offset = usize>> fmt::Display for EvaluatorValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            EvaluatorValue::Value(val, _) => val.fmt(f),
            EvaluatorValue::Bytes(byt) => write!(f, "{:?}", byt),
            EvaluatorValue::Array(arr) => arr.fmt(f),
            EvaluatorValue::Struct(stu) => stu.fmt(f),
            EvaluatorValue::Enum(enu) => enu.fmt(f),
            EvaluatorValue::Union(uni) => uni.fmt(f),
            EvaluatorValue::Member(mem) => mem.fmt(f),
            EvaluatorValue::Name(nam) => nam.fmt(f),
            EvaluatorValue::OutOfRange => write!(f, "< OutOfRange >"),
            EvaluatorValue::OptimizedOut => write!(f, "< OptimizedOut >"),
            EvaluatorValue::ZeroSize => write!(f, "< ZeroSize >"),
        };
    }
}

impl<R: Reader<Offset = usize>> EvaluatorValue<R> {
    pub fn to_value(self) -> Option<BaseValue> {
        match self {
            EvaluatorValue::Value(val, _) => Some(val),
            EvaluatorValue::Member(val) => val.value.to_value(),
            EvaluatorValue::OutOfRange => None,
            EvaluatorValue::OptimizedOut => None,
            EvaluatorValue::ZeroSize => None,
            _ => None, // TODO: Find a better solution then this.
        }
    }

    pub fn get_type(&self) -> String {
        match self {
            EvaluatorValue::Value(val, _) => val.get_type(),
            EvaluatorValue::Array(arr) => arr.get_type(),
            EvaluatorValue::Struct(stu) => stu.get_type(),
            EvaluatorValue::Enum(enu) => enu.get_type(),
            EvaluatorValue::Union(uni) => uni.get_type(),
            EvaluatorValue::Member(mem) => mem.get_type(),
            EvaluatorValue::Name(nam) => nam.to_string(),
            _ => "<unknown>".to_owned(),
        }
    }

    pub fn get_variable_information(self) -> Vec<ValueInformation> {
        match self {
            EvaluatorValue::Value(_, var_info) => vec![var_info],
            EvaluatorValue::Array(arr) => {
                let mut info = vec![];
                for val in arr.values {
                    info.append(&mut val.get_variable_information());
                }
                info
            }
            EvaluatorValue::Struct(st) => {
                let mut info = vec![];
                for val in st.members {
                    info.append(&mut val.get_variable_information());
                }
                info
            }
            EvaluatorValue::Enum(en) => en.value.get_variable_information(),
            EvaluatorValue::Union(un) => {
                let mut info = vec![];
                for val in un.members {
                    info.append(&mut val.get_variable_information());
                }
                info
            }
            EvaluatorValue::Member(me) => me.value.get_variable_information(),
            EvaluatorValue::OptimizedOut => {
                vec![ValueInformation::new(
                    None,
                    vec![ValuePiece::Dwarf { value: None }],
                )]
            }
            EvaluatorValue::OutOfRange => {
                vec![ValueInformation::new(
                    None,
                    vec![ValuePiece::Dwarf { value: None }],
                )]
            }
            _ => vec![],
        }
    }

    pub fn evaluate_variable_with_type<M: MemoryAccess>(
        dwarf: &gimli::Dwarf<R>,
        registers: &Registers,
        mem: &mut M,
        pieces: &Vec<Piece<R>>,
        unit_offset: gimli::UnitSectionOffset,
        die_offset: gimli::UnitOffset,
    ) -> Result<EvaluatorValue<R>> {
        // Initialize the memory offset to 0.
        let data_offset: u64 = 0;

        // Get the unit of the current state.
        let unit = match unit_offset {
            gimli::UnitSectionOffset::DebugInfoOffset(offset) => {
                let header = dwarf.debug_info.header_from_offset(offset)?;
                dwarf.unit(header)?
            }
            gimli::UnitSectionOffset::DebugTypesOffset(_offset) => {
                let mut iter = dwarf.debug_types.units();
                let mut result = None;
                while let Some(header) = iter.next()? {
                    if header.offset() == unit_offset {
                        result = Some(dwarf.unit(header)?);
                        break;
                    }
                }
                match result {
                    Some(val) => val,
                    None => bail!("Could not find unit form offset"),
                }
            }
        };

        // Get the die of the current state.
        let die = &unit.entry(die_offset)?;

        // Continue evaluating the value of the current state.
        EvaluatorValue::eval_type(
            registers,
            mem,
            dwarf,
            &unit,
            die,
            data_offset,
            pieces,
            &mut 0,
        )
    }

    pub fn evaluate_variable<M: MemoryAccess>(
        registers: &Registers,
        mem: &mut M,
        pieces: &Vec<Piece<R>>,
    ) -> Result<EvaluatorValue<R>> {
        EvaluatorValue::handle_eval_piece(registers, mem, 4, 0, DwAte(1), pieces, &mut 0)
    }

    /*
     * Evaluates the value of a piece and decides if the piece should be discarded or kept.
     */
    fn handle_eval_piece<M: MemoryAccess>(
        registers: &Registers,
        mem: &mut M,
        byte_size: u64,
        mut data_offset: u64,
        encoding: DwAte,
        pieces: &Vec<Piece<R>>,
        piece_index: &mut usize,
    ) -> Result<EvaluatorValue<R>> {
        if pieces.len() <= *piece_index {
            return Ok(EvaluatorValue::OptimizedOut);
        }

        // TODO: confirm
        if pieces.len() > 1 {
            // NOTE: Is this correct?
            data_offset = 0;
        }

        // TODO: confirm if this is correct
        if pieces.len() > 1 {
            data_offset = 0;
        }

        let mut all_bytes = vec![];
        let mut value_pieces = vec![];
        while all_bytes.len() < byte_size.try_into()? {
            if pieces.len() <= *piece_index {
                unreachable!();
                //return Ok(EvaluatorValue::OptimizedOut);
            }
            let piece = pieces[*piece_index].clone();

            // Evaluate the bytes needed from one gimli::Piece.
            match piece.location {
                Location::Empty => return Ok(EvaluatorValue::OptimizedOut),
                Location::Register { ref register } => {
                    match registers.get_register_value(&register.0) {
                        Some(val) => {
                            // TODO: Mask the important bits?
                            let mut bytes = vec![];
                            bytes.extend_from_slice(&val.to_le_bytes());

                            bytes = trim_piece_bytes(bytes, &piece, 4);
                            let byte_size = bytes.len();

                            all_bytes.extend_from_slice(&bytes);
                            value_pieces.extend_from_slice(&vec![ValuePiece::Register {
                                register: register.0,
                                byte_size: byte_size,
                            }]);
                        }
                        None => return Err(anyhow!("Requires reg")),
                    };
                }
                Location::Address { mut address } => {
                    address += data_offset;

                    let num_bytes = match piece.size_in_bits {
                        Some(val) => (val + 8 - 1) / 8,
                        None => byte_size,
                    } as usize;

                    let bytes = match mem.get_address(&(address as u32), num_bytes) {
                        Some(val) => val,
                        None => panic!("Return error"),
                    };

                    all_bytes.extend_from_slice(&bytes);
                    value_pieces.extend_from_slice(&vec![ValuePiece::Memory {
                        address: address as u32,
                        byte_size: num_bytes,
                    }]);
                    if pieces[*piece_index].size_in_bits.is_some() {
                        *piece_index += 1;
                    }
                }
                Location::Value { value } => {
                    return Ok(EvaluatorValue::Value(
                        convert_from_gimli_value(value),
                        ValueInformation {
                            raw: None,
                            pieces: vec![ValuePiece::Dwarf { value: Some(value) }],
                        },
                    ));
                }

                Location::Bytes { value } => return Ok(EvaluatorValue::Bytes(value.clone())),
                Location::ImplicitPointer {
                    value: _,
                    byte_offset: _,
                } => unimplemented!(),
            }
        }

        //        while bytes.len() > byte_size as usize {
        //            bytes.pop();    // TODO: Think this loop can be removed
        //        }

        Ok(EvaluatorValue::Value(
            BaseValue::parse_base_type(all_bytes.clone(), encoding)?,
            ValueInformation::new(Some(all_bytes.clone()), value_pieces),
        ))
    }

    /*
     * Evaluates the value of a type.
     */
    fn eval_type<M: MemoryAccess>(
        registers: &Registers,
        mem: &mut M,
        dwarf: &gimli::Dwarf<R>,
        unit: &gimli::Unit<R>,
        die: &gimli::DebuggingInformationEntry<'_, '_, R>,
        data_offset: u64,
        pieces: &Vec<Piece<R>>,
        piece_index: &mut usize,
    ) -> Result<EvaluatorValue<R>> {
        match die.tag() {
            gimli::DW_TAG_base_type => {
                // Make sure that the die has the tag DW_TAG_base_type.
                match die.tag() {
                    gimli::DW_TAG_base_type => (),
                    _ => bail!("Expected DW_TAG_base_type die, this should never happen"),
                };

                check_alignment(die, data_offset, pieces, piece_index)?;

                // Get byte size and encoding from the die.
                let byte_size = attributes::byte_size_attribute(die)
                    .ok_or(anyhow!("Expected to have byte_size attribute"))?;
                if byte_size == 0 {
                    return Ok(EvaluatorValue::ZeroSize);
                }
                let encoding = attributes::encoding_attribute(die)
                    .ok_or(anyhow!("Expected to habe encoding attribute"))?;

                // Evaluate the value.
                EvaluatorValue::handle_eval_piece(
                    registers,
                    mem,
                    byte_size,
                    data_offset, // TODO
                    encoding,
                    pieces,
                    piece_index,
                )
            }
            gimli::DW_TAG_pointer_type => {
                // Make sure that the die has the tag DW_TAG_array_type.
                match die.tag() {
                    gimli::DW_TAG_pointer_type => (),
                    _ => bail!("Expected DW_TAG_pointer_type die, this should never happen"),
                };

                check_alignment(die, data_offset, pieces, piece_index)?;

                // Evaluate the pointer type value.
                let address_class = match attributes::address_class_attribute(die) {
                    Some(val) => val,
                    None => bail!("Die is missing required attribute DW_AT_address_class"),
                };
                match address_class.0 {
                    0 => {
                        let res = EvaluatorValue::handle_eval_piece(
                            registers,
                            mem,
                            4, // This Should be set dependent on the system(4 for 32 bit systems)
                            data_offset,
                            DwAte(1),
                            pieces,
                            piece_index,
                        )?;
                        return Ok(res);
                    }
                    _ => bail!("Unimplemented DwAddr code"), // NOTE: The codes are architecture specific.
                };
            }
            gimli::DW_TAG_array_type => {
                // Make sure that the die has the tag DW_TAG_array_type.
                match die.tag() {
                    gimli::DW_TAG_array_type => (),
                    _ => bail!("Expected DW_TAG_array_type die, this should never happen"),
                };

                check_alignment(die, data_offset, pieces, piece_index)?;

                let children = get_children(unit, die)?;
                let dimension_die = unit.entry(children[0])?;

                let value = match dimension_die.tag() {
                    gimli::DW_TAG_subrange_type => EvaluatorValue::eval_type(
                        registers,
                        mem,
                        dwarf,
                        unit,
                        &dimension_die,
                        data_offset,
                        pieces,
                        piece_index,
                    )?,
                    gimli::DW_TAG_enumeration_type => EvaluatorValue::eval_type(
                        registers,
                        mem,
                        dwarf,
                        unit,
                        &dimension_die,
                        data_offset,
                        pieces,
                        piece_index,
                    )?,
                    _ => unimplemented!(),
                };

                // Evaluate the length of the array.
                let count = get_udata(match value.to_value() {
                    Some(val) => val,
                    None => return Ok(EvaluatorValue::OptimizedOut), // TODO: Maybe need to remove the following pieces that is related to this structure.
                }) as usize;

                // Get type attribute unit and die.
                let (type_unit, die_offset) = get_type_info(dwarf, unit, die)?;
                let type_die = &type_unit.entry(die_offset)?;

                // Evaluate all the values in the array.
                let mut values = vec![];
                for _i in 0..count {
                    values.push(EvaluatorValue::eval_type(
                        registers,
                        mem,
                        dwarf,
                        &type_unit,
                        type_die,
                        data_offset,
                        pieces,
                        piece_index,
                    )?);
                }

                Ok(EvaluatorValue::Array(Box::new(ArrayValue { values })))
            }
            gimli::DW_TAG_structure_type => {
                // Make sure that the die has the tag DW_TAG_structure_type.
                match die.tag() {
                    gimli::DW_TAG_structure_type => (),
                    _ => bail!("Expected DW_TAG_structure_type die, this should never happen"),
                };

                check_alignment(die, data_offset, pieces, piece_index)?;

                let name = match attributes::name_attribute(dwarf, die) {
                    Some(val) => val,
                    None => bail!("Expected the structure type die to have a name attribute"),
                };

                // Get all the DW_TAG_member dies.
                let children = get_children(unit, die)?;
                let mut member_dies = Vec::new();
                for c in &children {
                    let c_die = unit.entry(*c)?;
                    match c_die.tag() {
                        // If it is a DW_TAG_variant_part die then it is a enum and only have on value.
                        gimli::DW_TAG_variant_part => {
                            // Get the value.
                            let members = vec![EvaluatorValue::eval_type(
                                registers,
                                mem,
                                dwarf,
                                unit,
                                &c_die,
                                data_offset,
                                pieces,
                                piece_index,
                            )?];

                            return Ok(EvaluatorValue::Struct(Box::new(StructValue {
                                name,
                                members,
                            })));
                        }
                        gimli::DW_TAG_member => {
                            let data_member_location =
                                match attributes::data_member_location_attribute(&c_die) {
                                    Some(val) => val,
                                    None => bail!(
                                "Expected member die to have attribute DW_AT_data_member_location"
                            ),
                                };
                            member_dies.push((data_member_location, c_die))
                        }
                        _ => continue,
                    };
                }

                // Sort the members in the evaluation order.
                member_dies.sort_by_key(|m| m.0);

                // Evaluate all the members.
                let mut members = vec![];
                for i in 0..member_dies.len() {
                    let m_die = &member_dies[i].1;
                    let member = match m_die.tag() {
                        gimli::DW_TAG_member => EvaluatorValue::eval_type(
                            registers,
                            mem,
                            dwarf,
                            unit,
                            m_die,
                            data_offset,
                            pieces,
                            piece_index,
                        )?,
                        _ => panic!("Unexpected die"),
                    };
                    members.push(member);
                }

                return Ok(EvaluatorValue::Struct(Box::new(StructValue {
                    name,
                    members,
                })));
            }
            gimli::DW_TAG_union_type => {
                // Make sure that the die has the tag DW_TAG_union_type.
                match die.tag() {
                    gimli::DW_TAG_union_type => (),
                    _ => bail!("Expected DW_TAG_union_type die, this should never happen"),
                };

                check_alignment(die, data_offset, pieces, piece_index)?;

                let name = match attributes::name_attribute(dwarf, die) {
                    Some(val) => val,
                    None => bail!("Expected union type die to have a name attribute"),
                };

                // Get all children of type DW_TAG_member.
                let children = get_children(unit, die)?;
                let mut member_dies = vec![];
                for c in children {
                    let c_die = unit.entry(c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_member => {
                            let data_member_location =
                                match attributes::data_member_location_attribute(&c_die) {
                                    Some(val) => val,
                                    None => bail!(
                                "Expected member die to have attribute DW_AT_data_member_location"
                            ),
                                };
                            member_dies.push((data_member_location, c_die))
                        }
                        _ => continue,
                    };
                }

                // Sort all the members in the order they need to be evaluated.
                member_dies.sort_by_key(|m| m.0);

                // Evaluate all the members.
                let mut members = vec![];
                for i in 0..member_dies.len() {
                    let m_die = &member_dies[i].1;
                    let member = match m_die.tag() {
                        gimli::DW_TAG_member => EvaluatorValue::eval_type(
                            registers,
                            mem,
                            dwarf,
                            unit,
                            m_die,
                            data_offset,
                            pieces,
                            piece_index,
                        )?,
                        _ => panic!("Unexpected die"),
                    };
                    members.push(member);
                }

                return Ok(EvaluatorValue::Union(Box::new(UnionValue {
                    name,
                    members,
                })));
            }
            gimli::DW_TAG_member => {
                // Make sure that the die has the tag DW_TAG_member
                match die.tag() {
                    gimli::DW_TAG_member => (),
                    _ => bail!("Expected DW_TAG_member die, this should never happen"),
                };

                // Get the name of the member.
                let name = attributes::name_attribute(dwarf, die);

                // Calculate the new data offset.
                let new_data_offset = match attributes::data_member_location_attribute(die) {
                    // NOTE: Seams it can also be a location description and not an offset. Dwarf 5 page 118
                    Some(val) => data_offset + val,
                    None => data_offset,
                };

                check_alignment(die, new_data_offset, pieces, piece_index)?;

                // Get the type attribute unit and die.
                let (type_unit, die_offset) = get_type_info(dwarf, unit, die)?;
                let type_die = &type_unit.entry(die_offset)?;

                // Evaluate the value.
                let value = EvaluatorValue::eval_type(
                    registers,
                    mem,
                    dwarf,
                    &type_unit,
                    type_die,
                    new_data_offset,
                    pieces,
                    piece_index,
                )?;

                Ok(EvaluatorValue::Member(Box::new(MemberValue {
                    name,
                    value,
                })))
            }
            gimli::DW_TAG_enumeration_type => {
                // Make sure that the die has the tag DW_TAG_enumeration_type
                match die.tag() {
                    gimli::DW_TAG_enumeration_type => (),
                    _ => bail!("Expected DW_TAG_enumeration_type die, this should never happen"),
                };

                check_alignment(die, data_offset, pieces, piece_index)?;

                // Get type attribute unit and die.
                let (type_unit, die_offset) = get_type_info(dwarf, unit, die)?;
                let type_die = &type_unit.entry(die_offset)?;

                // Get type value.
                let type_result = EvaluatorValue::eval_type(
                    registers,
                    mem,
                    dwarf,
                    &type_unit,
                    type_die,
                    data_offset,
                    pieces,
                    piece_index,
                )?;

                // Get the value as a unsigned int.
                let value = get_udata(match type_result.to_value() {
                    Some(val) => val,
                    None => return Ok(EvaluatorValue::OptimizedOut), // TODO: Maybe need to remove the following pieces that is related to this structure.
                });

                // Go through the children and find the correct enumerator value.
                let children = get_children(unit, die)?;

                let clen = children.len() as u64;

                for c in children {
                    let c_die = unit.entry(c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_enumerator => {
                            let const_value = match attributes::const_value_attribute(&c_die) {
                                Some(val) => val,
                                None => bail!(
                                "Expected enumeration type die to have attribute DW_AT_const_value"
                            ),
                            };

                            // Check if it is the correct one.
                            if const_value == value % clen {
                                // Get the name of the enum type and the enum variant.
                                let name = match attributes::name_attribute(dwarf, die) {
                                    Some(val) => val,
                                    None => {
                                        bail!("Expected enumeration type die to have attribute DW_AT_name")
                                    }
                                };

                                let e_name = match attributes::name_attribute(dwarf, &c_die) {
                                    Some(val) => val,
                                    None => {
                                        bail!(
                                            "Expected enumerator die to have attribute DW_AT_name"
                                        )
                                    }
                                };

                                return Ok(EvaluatorValue::Enum(Box::new(EnumValue {
                                    name,
                                    value: EvaluatorValue::Name(e_name),
                                })));
                            }
                        }
                        gimli::DW_TAG_subprogram => (),
                        _ => unimplemented!(),
                    };
                }

                unreachable!()
            }
            gimli::DW_TAG_variant_part => {
                // Make sure that the die has tag DW_TAG_variant_part
                match die.tag() {
                    gimli::DW_TAG_variant_part => (),
                    _ => bail!("Expected DW_TAG_variant_part die, this should never happen"),
                };

                check_alignment(die, data_offset, pieces, piece_index)?;

                // Get the enum variant.
                // TODO: If variant is optimised out then return optimised out and remove the pieces for
                // this type if needed.

                // Get member die.
                let die_offset = match attributes::discr_attribute(die) {
                    Some(val) => val,
                    None => bail!("Expected variant part die to have attribute DW_AT_discr"),
                };
                let member = &unit.entry(die_offset)?;

                // Evaluate the DW_TAG_member value.
                let value = match member.tag() {
                    gimli::DW_TAG_member => EvaluatorValue::eval_type(
                        registers,
                        mem,
                        dwarf,
                        unit,
                        member,
                        data_offset,
                        pieces,
                        piece_index,
                    )?,
                    _ => panic!("Unexpected"),
                };

                // The value should be a unsigned int thus convert the value to a u64.
                let variant = get_udata(match value.to_value() {
                    Some(val) => val,
                    None => return Ok(EvaluatorValue::OptimizedOut), // TODO: Maybe need to remove the following pieces that is related to this structure.
                });

                // Find the DW_TAG_member die and all the DW_TAG_variant dies.
                let mut variants = vec![];
                let children = get_children(unit, die)?;
                for c in &children {
                    let c_die = unit.entry(*c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_variant => {
                            variants.push(c_die);
                        }
                        _ => (),
                    };
                }

                for v in &variants {
                    // Find the right variant type and evaluate it.
                    let discr_value = match attributes::discr_value_attribute(v) {
                        Some(val) => val,
                        None => bail!("Expected variant die to have attribute DW_AT_discr_value"),
                    };

                    // Check if it is the correct variant.
                    if discr_value == variant % (variants.len() as u64) {
                        // NOTE: Don't know if using modulus here is correct, but it seems to be correct.

                        // Evaluate the value of the variant.
                        match v.tag() {
                            gimli::DW_TAG_variant => {
                                return EvaluatorValue::eval_type(
                                    registers,
                                    mem,
                                    dwarf,
                                    unit,
                                    v,
                                    data_offset,
                                    pieces,
                                    piece_index,
                                );
                            }
                            _ => panic!("Expected variant die"),
                        };
                    }
                }

                unreachable!();
            }
            gimli::DW_TAG_variant => {
                check_alignment(die, data_offset, pieces, piece_index)?;

                // Find the child die of type DW_TAG_member
                let children = get_children(unit, die)?;
                for c in children {
                    let c_die = unit.entry(c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_member => {
                            // Evaluate the value of the member.
                            let value = EvaluatorValue::eval_type(
                                registers,
                                mem,
                                dwarf,
                                unit,
                                &c_die,
                                data_offset,
                                pieces,
                                piece_index,
                            )?;

                            // Get the name of the die.
                            let name = match attributes::name_attribute(dwarf, &c_die) {
                                Some(val) => val,
                                None => bail!("Expected member die to have attribute DW_AT_name"),
                            };

                            return Ok(EvaluatorValue::Enum(Box::new(EnumValue { name, value })));
                        }
                        _ => (),
                    };
                }

                unreachable!();
            }
            gimli::DW_TAG_subrange_type => {
                // Make sure that the die has the tag DW_TAG_subrange_type
                match die.tag() {
                    gimli::DW_TAG_subrange_type => (),
                    _ => bail!("Expected DW_TAG_subrange_type die, this should never happen"),
                };

                // If the die has a count attribute then that is the value.
                match attributes::count_attribute(die) {
                    // NOTE: This could be replace with lower and upper bound
                    Some(val) => {
                        return Ok(EvaluatorValue::Value(
                            BaseValue::U64(val),
                            ValueInformation::new(None, vec![ValuePiece::Dwarf { value: None }]),
                        ))
                    }
                    None => (),
                };

                // Get the type unit and die.
                let (type_unit, die_offset) = match get_type_info(dwarf, unit, die) {
                    Ok(val) => val,
                    Err(_) => bail!("Expected subrange type die to have type information"),
                };
                let type_die = &type_unit.entry(die_offset)?;

                // Evaluate the type attribute value.
                Ok(EvaluatorValue::eval_type(
                    registers,
                    mem,
                    dwarf,
                    &type_unit,
                    type_die,
                    data_offset,
                    pieces,
                    piece_index,
                )?)
            }
            gimli::DW_TAG_subroutine_type => unimplemented!(),
            gimli::DW_TAG_subprogram => unimplemented!(),
            gimli::DW_TAG_string_type => unimplemented!(),
            gimli::DW_TAG_generic_subrange => unimplemented!(),
            gimli::DW_TAG_template_type_parameter => unimplemented!(),
            _ => unimplemented!(),
        }
    }
}

fn get_udata(value: BaseValue) -> u64 {
    match value {
        BaseValue::U8(v) => v as u64,
        BaseValue::U16(v) => v as u64,
        BaseValue::U32(v) => v as u64,
        BaseValue::U64(v) => v,
        BaseValue::Generic(v) => v,
        _ => unimplemented!(),
    }
}

fn format_values<R: Reader<Offset = usize>>(values: &Vec<EvaluatorValue<R>>) -> String {
    let len = values.len();
    if len == 0 {
        return "".to_string();
    } else if len == 1 {
        return format!("{}", values[0]);
    }

    let mut res = format!("{}", values[0]);
    for i in 1..len {
        res = format!("{}, {}", res, values[i]);
    }
    return res;
}

fn format_types<R: Reader<Offset = usize>>(values: &Vec<EvaluatorValue<R>>) -> String {
    let len = values.len();
    if len == 0 {
        return "".to_string();
    } else if len == 1 {
        return format!("{}", values[0].get_type());
    }

    let mut res = format!("{}", values[0].get_type());
    for i in 1..len {
        res = format!("{}, {}", res, values[i].get_type());
    }
    return res;
}

#[derive(Debug, Clone)]
pub struct ArrayValue<R: Reader<Offset = usize>> {
    pub values: Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for ArrayValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ {} ]", format_values(&self.values))
    }
}

impl<R: Reader<Offset = usize>> ArrayValue<R> {
    pub fn get_type(&self) -> String {
        format!("[ {} ]", format_types(&self.values))
    }
}

#[derive(Debug, Clone)]
pub struct StructValue<R: Reader<Offset = usize>> {
    pub name: String,
    pub members: Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for StructValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {{ {} }}", self.name, format_values(&self.members))
    }
}

impl<R: Reader<Offset = usize>> StructValue<R> {
    pub fn get_type(&self) -> String {
        format!("{} {{ {} }}", self.name, format_types(&self.members))
    }
}

#[derive(Debug, Clone)]
pub struct EnumValue<R: Reader<Offset = usize>> {
    pub name: String,
    pub value: EvaluatorValue<R>,
}

impl<R: Reader<Offset = usize>> fmt::Display for EnumValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}", self.name, self.value)
    }
}

impl<R: Reader<Offset = usize>> EnumValue<R> {
    pub fn get_type(&self) -> String {
        format!("{}::{}", self.name, self.value.get_type())
    }
}

#[derive(Debug, Clone)]
pub struct UnionValue<R: Reader<Offset = usize>> {
    pub name: String,
    pub members: Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for UnionValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ( {} )", self.name, format_values(&self.members))
    }
}

impl<R: Reader<Offset = usize>> UnionValue<R> {
    pub fn get_type(&self) -> String {
        format!("{} ( {} )", self.name, format_types(&self.members))
    }
}

#[derive(Debug, Clone)]
pub struct MemberValue<R: Reader<Offset = usize>> {
    pub name: Option<String>,
    pub value: EvaluatorValue<R>,
}

impl<R: Reader<Offset = usize>> fmt::Display for MemberValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match &self.name {
            Some(name) => write!(f, "{}::{}", name, self.value),
            None => write!(f, "{}", self.value),
        };
    }
}

impl<R: Reader<Offset = usize>> MemberValue<R> {
    pub fn get_type(&self) -> String {
        match &self.name {
            Some(name) => format!("{}::{}", name, self.value.get_type()),
            None => format!("{}", self.value.get_type()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum BaseValue {
    Generic(u64),

    Address32(u32),
    Bool(bool),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),

    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),

    F32(f32),
    F64(f64),
}

impl fmt::Display for BaseValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            BaseValue::Bool(val) => write!(f, "{}", val),
            BaseValue::Generic(val) => write!(f, "{}", val),
            BaseValue::I8(val) => write!(f, "{}", val),
            BaseValue::U8(val) => write!(f, "{}", val),
            BaseValue::I16(val) => write!(f, "{}", val),
            BaseValue::U16(val) => write!(f, "{}", val),
            BaseValue::I32(val) => write!(f, "{}", val),
            BaseValue::U32(val) => write!(f, "{}", val),
            BaseValue::I64(val) => write!(f, "{}", val),
            BaseValue::U64(val) => write!(f, "{}", val),
            BaseValue::F32(val) => write!(f, "{}", val),
            BaseValue::F64(val) => write!(f, "{}", val),
            BaseValue::Address32(val) => write!(f, "'Address' {:#10x}", val),
        };
    }
}

impl BaseValue {
    pub fn parse_base_type(data: Vec<u8>, encoding: DwAte) -> Result<BaseValue> {
        if data.len() == 0 {
            return Err(anyhow!("Expected data to be larger then 0"));
        }

        Ok(match (encoding, data.len()) {
            // Source: DWARF 4 page 168-169 and 77
            (DwAte(1), 4) => BaseValue::Address32(u32::from_le_bytes(data.try_into().unwrap())), // DW_ATE_address = 1 // TODO: Different size addresses?
            (DwAte(2), 1) => BaseValue::Bool((u8::from_le_bytes(data.try_into().unwrap())) == 1), // DW_ATE_boolean = 2 // TODO: Use modulus?

            //        (DwAte(3), _) => ,   // DW_ATE_complex_float = 3 // NOTE: Seems like a C++ thing
            (DwAte(4), 4) => BaseValue::F32(f32::from_le_bytes(data.try_into().unwrap())), // DW_ATE_float = 4
            (DwAte(4), 8) => BaseValue::F64(f64::from_le_bytes(data.try_into().unwrap())), // DW_ATE_float = 4

            (DwAte(5), 1) => BaseValue::I8(i8::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_signed = 5, 8)
            (DwAte(5), 2) => BaseValue::I16(i16::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_signed = 5, 16)
            (DwAte(5), 4) => BaseValue::I32(i32::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_signed = 5, 32)
            (DwAte(5), 8) => BaseValue::I64(i64::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_signed = 5, 64)

            //        (DwAte(6), _) => ,     // DW_ATE_signed_char = 6 // TODO: Add type
            (DwAte(7), 1) => BaseValue::U8(u8::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_unsigned = 7, 8)
            (DwAte(7), 2) => BaseValue::U16(u16::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_unsigned = 7, 16)
            (DwAte(7), 4) => BaseValue::U32(u32::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_unsigned = 7, 32)
            (DwAte(7), 8) => BaseValue::U64(u64::from_le_bytes(data.try_into().unwrap())), // (DW_ATE_unsigned = 7, 64)
            _ => {
                unimplemented!("encoding {}, byte_size: {}", encoding, data.len());
            }
        })
    }

    pub fn get_type(&self) -> String {
        match self {
            BaseValue::Bool(_) => "bool".to_owned(),
            BaseValue::Generic(_) => "<unknown>".to_owned(),
            BaseValue::I8(_) => "i8".to_owned(),
            BaseValue::U8(_) => "u8".to_owned(),
            BaseValue::I16(_) => "i16".to_owned(),
            BaseValue::U16(_) => "u16".to_owned(),
            BaseValue::I32(_) => "i32".to_owned(),
            BaseValue::U32(_) => "u32".to_owned(),
            BaseValue::I64(_) => "i64".to_owned(),
            BaseValue::U64(_) => "u64".to_owned(),
            BaseValue::F32(_) => "f32".to_owned(),
            BaseValue::F64(_) => "f63".to_owned(),
            BaseValue::Address32(_) => "<32 bit address>".to_owned(),
        }
    }
}

pub fn convert_to_gimli_value(value: BaseValue) -> gimli::Value {
    match value {
        BaseValue::Bool(val) => gimli::Value::Generic(match val {
            true => 1,
            false => 0,
        }),
        BaseValue::Generic(val) => gimli::Value::Generic(val),
        BaseValue::I8(val) => gimli::Value::I8(val),
        BaseValue::U8(val) => gimli::Value::U8(val),
        BaseValue::I16(val) => gimli::Value::I16(val),
        BaseValue::U16(val) => gimli::Value::U16(val),
        BaseValue::I32(val) => gimli::Value::I32(val),
        BaseValue::U32(val) => gimli::Value::U32(val),
        BaseValue::I64(val) => gimli::Value::I64(val),
        BaseValue::U64(val) => gimli::Value::U64(val),
        BaseValue::F32(val) => gimli::Value::F32(val),
        BaseValue::F64(val) => gimli::Value::F64(val),
        BaseValue::Address32(val) => gimli::Value::Generic(val as u64),
    }
}

pub fn convert_from_gimli_value(value: gimli::Value) -> BaseValue {
    match value {
        gimli::Value::Generic(val) => BaseValue::Generic(val),
        gimli::Value::I8(val) => BaseValue::I8(val),
        gimli::Value::U8(val) => BaseValue::U8(val),
        gimli::Value::I16(val) => BaseValue::I16(val),
        gimli::Value::U16(val) => BaseValue::U16(val),
        gimli::Value::I32(val) => BaseValue::I32(val),
        gimli::Value::U32(val) => BaseValue::U32(val),
        gimli::Value::I64(val) => BaseValue::I64(val),
        gimli::Value::U64(val) => BaseValue::U64(val),
        gimli::Value::F32(val) => BaseValue::F32(val),
        gimli::Value::F64(val) => BaseValue::F64(val),
    }
}

/*
 * Helper method for getting the unit and die from the type attribute of the current die.
 */
fn get_type_info<R: Reader<Offset = usize>>(
    dwarf: &gimli::Dwarf<R>,
    unit: &gimli::Unit<R>,
    die: &gimli::DebuggingInformationEntry<'_, '_, R>,
) -> Result<(gimli::Unit<R>, gimli::UnitOffset)> {
    let (unit_offset, die_offset) = match attributes::type_attribute(dwarf, unit, die)? {
        Some(val) => val,
        None => bail!("Die doesn't have the required DW_AT_type attribute"),
    };
    let unit = match unit_offset {
        gimli::UnitSectionOffset::DebugInfoOffset(offset) => {
            let header = dwarf.debug_info.header_from_offset(offset)?;
            dwarf.unit(header)?
        }
        gimli::UnitSectionOffset::DebugTypesOffset(_offset) => {
            let mut iter = dwarf.debug_types.units();
            let mut result = None;
            while let Some(header) = iter.next()? {
                if header.offset() == unit_offset {
                    result = Some(dwarf.unit(header)?);
                    break;
                }
            }
            match result {
                Some(val) => val,
                None => bail!("Could not get unit from unit offset"),
            }
        }
    };

    Ok((unit, die_offset))
}

/*
 * Check if address is correctly aligned
 *
 * NOTE: Don't know if it is correct.
 */
fn check_alignment<R: Reader<Offset = usize>>(
    die: &gimli::DebuggingInformationEntry<'_, '_, R>,
    mut data_offset: u64,
    pieces: &Vec<Piece<R>>,
    piece_index: &mut usize,
) -> Result<()> {
    match attributes::alignment_attribute(die) {
        Some(alignment) => {
            if pieces.len() <= *piece_index {
                return Ok(());
            }

            if pieces.len() < 1 {
                data_offset = 0;
            }

            match pieces[*piece_index].location {
                Location::Address { address } => {
                    let mut addr = address + (data_offset / 4) * 4;
                    addr -= addr % 4; // TODO: Is this correct?

                    if addr % alignment != 0 {
                        bail!("Address not aligned");
                    }
                }
                _ => (),
            };
        }
        None => (),
    };

    Ok(())
}

/*
 * Helper function for getting all the children of a die.
 */
fn get_children<R: Reader<Offset = usize>>(
    unit: &gimli::Unit<R>,
    die: &gimli::DebuggingInformationEntry<'_, '_, R>,
) -> Result<Vec<gimli::UnitOffset>> {
    let mut result = Vec::new();
    let mut tree = unit.entries_tree(Some(die.offset()))?;
    let node = tree.root()?;

    let mut children = node.children();
    while let Some(child) = children.next()? {
        result.push(child.entry().offset());
    }

    Ok(result)
}

fn trim_piece_bytes<R: Reader<Offset = usize>>(
    mut bytes: Vec<u8>,
    piece: &Piece<R>,
    byte_size: usize,
) -> Vec<u8> {
    let piece_byte_size = match piece.size_in_bits {
        Some(size) => ((size + 8 - 1) / 8) as usize,
        None => byte_size,
    };

    let piece_byte_offset = match piece.bit_offset {
        Some(offset) => {
            //if offset % 8 == 0 {
            //    panic!("Expected the offset to be in bytes, got {} bits", offset);
            //}
            ((offset + 8 - 1) / 8) as usize
        }
        None => 0,
    };

    for _ in 0..piece_byte_offset {
        bytes.pop();
    }

    while bytes.len() > piece_byte_size {
        // TODO: Check that this follows the ABI.
        bytes.remove(0);
    }

    return bytes;
}

#[derive(Debug, Clone)]
pub struct ValueInformation {
    pub raw: Option<Vec<u8>>, // byte size and raw value
    pub pieces: Vec<ValuePiece>,
}

impl ValueInformation {
    pub fn new(raw: Option<Vec<u8>>, pieces: Vec<ValuePiece>) -> ValueInformation {
        ValueInformation { raw, pieces }
    }
}

#[derive(Debug, Clone)]
pub enum ValuePiece {
    Register { register: u16, byte_size: usize },
    Memory { address: u32, byte_size: usize },
    Dwarf { value: Option<gimli::Value> },
}
