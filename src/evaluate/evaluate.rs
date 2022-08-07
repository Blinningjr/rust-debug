use super::attributes;
use crate::call_stack::MemoryAccess;
use crate::registers::Registers;
use std::convert::TryInto;

use gimli::{DwAte, Location, Piece, Reader};

use anyhow::{anyhow, Result};
use log::{info, error};

use std::fmt;

/// A wrapper for `gimli::Piece` which also contains a boolean that describes if this piece has
/// already been used to evaluate a value.
/// This means that the offset in the type information should be used.
#[derive(Debug, Clone)]
struct MyPiece<R: Reader<Offset = usize>> {
    /// The piece which contains location information.
    pub piece: Piece<R>,

    /// Is true if this piece has already been used to evaluate a value.
    pub used_before: bool,
}
impl<R: Reader<Offset = usize>> MyPiece<R> {
    /// Creates a new `MyPiece`.
    pub fn new(piece: Piece<R>) -> MyPiece<R> {
        MyPiece {
            piece,
            used_before: false,
        }
    }

    /// Updates the size in_bits value and return a boolean which tells if the piece is consumed
    /// and should be removed.
    ///
    /// Description:
    ///
    /// * `bit_size` - How many bits of data needed from the piece.
    pub fn should_remove(&mut self, bit_size: u64) -> bool {
        match self.piece.size_in_bits {
            Some(val) => {
                if val > bit_size {
                    self.piece.size_in_bits = Some(val - bit_size);
                    self.used_before = true;
                    false
                } else {
                    self.used_before = true;
                    self.piece.size_in_bits = Some(0);
                    true
                }
            }
            None => {
                self.used_before = true;
                false
            }
        }
    }
}

/// Describes all the different Rust types values in the form of a tree structure.
#[derive(Debug, Clone)]
pub enum EvaluatorValue<R: Reader<Offset = usize>> {
    /// A base_type type and value with location information.
    Value(BaseTypeValue, ValueInformation),

    /// A pointer_type type and value.
    PointerTypeValue(Box<PointerTypeValue<R>>),

    /// A variant type and value.
    VariantValue(Box<VariantValue<R>>),

    /// A variant_part type and value.
    VariantPartValue(Box<VariantPartValue<R>>),

    /// A subrange_type type and value.
    SubrangeTypeValue(SubrangeTypeValue),

    /// gimli-rs bytes value.
    Bytes(R),

    /// A array type value.
    Array(Box<ArrayTypeValue<R>>),

    /// A struct type value.
    Struct(Box<StructureTypeValue<R>>),

    /// A enum type value.
    Enum(Box<EnumerationTypeValue<R>>),

    /// A union type value.
    Union(Box<UnionTypeValue<R>>),

    /// A attribute type value.
    Member(Box<MemberValue<R>>),

    /// The value is optimized away.
    OptimizedOut, // NOTE: Value is optimized out.

    /// The variable has no location currently but had or will have one. Note that the location can
    /// be a constant stored in the DWARF stack.
    LocationOutOfRange,

    /// The value is size 0 bits.
    ZeroSize,
}

impl<R: Reader<Offset = usize>> fmt::Display for EvaluatorValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            EvaluatorValue::Value(val, _) => val.fmt(f),
            EvaluatorValue::PointerTypeValue(pt) => pt.fmt(f),
            EvaluatorValue::VariantValue(var) => var.fmt(f),
            EvaluatorValue::VariantPartValue(vpa) => vpa.fmt(f),
            EvaluatorValue::SubrangeTypeValue(srt) => srt.fmt(f),
            EvaluatorValue::Bytes(byt) => write!(f, "{:?}", byt),
            EvaluatorValue::Array(arr) => arr.fmt(f),
            EvaluatorValue::Struct(stu) => stu.fmt(f),
            EvaluatorValue::Enum(enu) => enu.fmt(f),
            EvaluatorValue::Union(uni) => uni.fmt(f),
            EvaluatorValue::Member(mem) => mem.fmt(f),
            EvaluatorValue::OptimizedOut => write!(f, "< OptimizedOut >"),
            EvaluatorValue::LocationOutOfRange => write!(f, "< LocationOutOfRange >"),
            EvaluatorValue::ZeroSize => write!(f, "< ZeroSize >"),
        };
    }
}

impl<R: Reader<Offset = usize>> EvaluatorValue<R> {
    /// Will return this value as a `BaseTypeValue` struct if possible.
    pub fn to_value(self) -> Option<BaseTypeValue> {
        match self {
            EvaluatorValue::Value(val, _) => Some(val),
            EvaluatorValue::Member(val) => val.value.to_value(),
            EvaluatorValue::OptimizedOut => None,
            EvaluatorValue::ZeroSize => None,
            _ => None, // TODO: Find a better solution then this.
        }
    }

    /// Will return the type of this value as a `String`.
    pub fn get_type(&self) -> String {
        match self {
            // TODO Update
            EvaluatorValue::Value(val, _) => val.get_type(),
            EvaluatorValue::Array(arr) => arr.get_type(),
            EvaluatorValue::Struct(stu) => stu.get_type(),
            EvaluatorValue::Enum(enu) => enu.get_type(),
            EvaluatorValue::Union(uni) => uni.get_type(),
            EvaluatorValue::Member(mem) => mem.get_type(),
            _ => "<unknown>".to_owned(),
        }
    }

    /// Will return a `Vec` of location and unparsed value infromation about the value.
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
            EvaluatorValue::Enum(en) => en.variant.get_variable_information(),
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
            _ => vec![],
        }
    }

    /// Evaluate a list of `Piece`s into a value and parse it to the given type.
    ///
    /// Description:
    ///
    /// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
    /// * `registers` - A register struct for accessing the register values.
    /// * `mem` - A struct for accessing the memory of the debug target.
    /// * `pieces` - A list of gimli-rs pieces containing the location information..
    /// * `unit_offset` - A offset to the `Unit` which contains the given type DIE.
    /// * `die_offset` - A offset to the DIE that contains the type of the value.
    ///
    /// This function will use the location information in the `pieces` parameter to read the
    /// values and parse it to the given type.
    pub fn evaluate_variable_with_type<M: MemoryAccess>(
        dwarf: &gimli::Dwarf<R>,
        registers: &Registers,
        mem: &mut M,
        pieces: &Vec<Piece<R>>,
        unit_offset: gimli::UnitSectionOffset,
        die_offset: gimli::UnitOffset,
    ) -> Result<EvaluatorValue<R>> {
        log::info!("evaluate_variable_with_type");
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
                    None => {
                        error!("Could not find unit from offset");
                        return Err(anyhow!("Could not find unit from offset"));
                    }
                }
            }
        };

        // Get the die of the current state.
        let die = &unit.entry(die_offset)?;

        let mut my_pieces = pieces.iter().map(|p| MyPiece::new(p.clone())).collect();

        // Continue evaluating the value of the current state.
        EvaluatorValue::eval_type(
            registers,
            mem,
            dwarf,
            &unit,
            die,
            data_offset,
            &mut my_pieces,
        )
    }

    /// This function will evaluate the given pieces into a unsigned 32 bit integer.
    ///
    /// Description:
    ///
    /// * `registers` - A register struct for accessing the register values.
    /// * `mem` - A struct for accessing the memory of the debug target.
    /// * `pieces` - A list of gimli-rs pieces containing the location information..
    pub fn evaluate_variable<M: MemoryAccess>(
        registers: &Registers,
        mem: &mut M,
        pieces: &Vec<Piece<R>>,
    ) -> Result<EvaluatorValue<R>> {
        log::info!("evaluate_variable");
        let mut my_pieces = pieces.iter().map(|p| MyPiece::new(p.clone())).collect();
        EvaluatorValue::handle_eval_piece(registers, mem, 4, 0, DwAte(1), &mut my_pieces)
    }

    /// Will maybe consume a number of pieces to evaluate a base type.
    ///
    /// Description:
    ///
    /// * `registers` - A register struct for accessing the register values.
    /// * `mem` - A struct for accessing the memory of the debug target.
    /// * `byte_size` - The size of the base type in bytes.
    /// * `data_offset` - The memory address offset.
    /// * `encoding` - The encoding of the base type.
    /// * `pieces` - A list of pieces containing the location and size information.
    fn handle_eval_piece<M: MemoryAccess>(
        registers: &Registers,
        mem: &mut M,
        byte_size: u64,
        data_offset: u64,
        encoding: DwAte,
        pieces: &mut Vec<MyPiece<R>>,
    ) -> Result<EvaluatorValue<R>> {
                info!("encoding: {:?}", encoding);
                info!("byte_size: {:?}", byte_size);
        if pieces.len() == 0 {
            return Ok(EvaluatorValue::OptimizedOut);
        }

        let mut all_bytes = vec![];
        let mut value_pieces = vec![];
        while all_bytes.len() < byte_size.try_into()? {
            if pieces.len() == 0 {
                error!("Unreachable");
                return Err(anyhow!("Unreachable"));
                //return Ok(EvaluatorValue::OptimizedOut);
            }

            // Evaluate the bytes needed from one gimli::Piece.
            match pieces[0].piece.clone().location {
                Location::Empty => {
                    // Remove piece if whole object is used.
                    let bit_size = 8 * (byte_size - all_bytes.len() as u64);
                    if pieces[0].should_remove(bit_size) {
                        pieces.remove(0);
                    }
                    return Ok(EvaluatorValue::OptimizedOut);
                }
                Location::Register { ref register } => {
                    match registers.get_register_value(&register.0) {
                        Some(val) => {
                            // TODO: Mask the important bits?
                            let mut bytes = vec![];
                            bytes.extend_from_slice(&val.to_le_bytes());

                            bytes = trim_piece_bytes(bytes, &pieces[0].piece, 4); // 4 because 32 bit registers
                            let bytes_len = bytes.len();

                            all_bytes.extend_from_slice(&bytes);
                            value_pieces.extend_from_slice(&vec![ValuePiece::Register {
                                register: register.0,
                                byte_size: bytes_len,
                            }]);

                            // Remove piece if whole object is used.
                            let bit_size = 8 * (bytes_len as u64);
                            if pieces[0].should_remove(bit_size) {
                                pieces.remove(0);
                            }
                        }
                        None => return Err(anyhow!("Requires reg")),
                    };
                }
                Location::Address { mut address } => {
                    // Check if `data_offset` should be used.
                    address += {
                        if pieces[0].used_before {
                            data_offset
                        } else {
                            0
                        }
                    };

                    let num_bytes = match pieces[0].piece.size_in_bits {
                        Some(val) => {
                            let max_num_bytes = (val + 8 - 1) / 8;
                            let needed_num_bytes = byte_size - all_bytes.len() as u64;
                            if max_num_bytes < needed_num_bytes {
                                max_num_bytes
                            } else {
                                needed_num_bytes
                            }
                        }
                        None => byte_size - all_bytes.len() as u64,
                    } as usize;

                    let bytes = match mem.get_address(&(address as u32), num_bytes) {
                        Some(val) => val,
                        None => {
                            error!(
                                "can not read address: {:x} num_bytes: {:?}, Return error",
                                address as u64, num_bytes
                            );
                            return Err(anyhow!(
                                "can not read address: {:x} num_bytes: {:?}, Return error",
                                address as u64,
                                num_bytes
                            ));
                        }
                    };

                    all_bytes.extend_from_slice(&bytes);
                    value_pieces.extend_from_slice(&vec![ValuePiece::Memory {
                        address: address as u32,
                        byte_size: num_bytes,
                    }]);

                    // Remove piece if whole object is used.
                    let bit_size = 8 * num_bytes as u64;
                    if pieces[0].should_remove(bit_size) {
                        pieces.remove(0);
                    }
                }
                Location::Value { value } => {
                    // Remove piece if whole object is used.
                    let bit_size = 8 * (byte_size - all_bytes.len() as u64);
                    if pieces[0].should_remove(bit_size) {
                        pieces.remove(0);
                    }

                    let parsed_value = convert_from_gimli_value(value);
                    return match parsed_value {
                        BaseTypeValue::Generic(v) => {
                            let correct_value = match (encoding, byte_size) {                                
                                (DwAte(1), 4) => BaseTypeValue::Address32(v as u32),
                                //(DwAte(1), 4) => BaseTypeValue::Reg32(v as u32),
                                (DwAte(2), _) => BaseTypeValue::Bool(v != 0),
                                (DwAte(7), 1) => BaseTypeValue::U8(v as u8),
                                (DwAte(7), 2) => BaseTypeValue::U16(v as u16),
                                (DwAte(7), 4) => BaseTypeValue::U32(v as u32),
                                (DwAte(7), 8) => BaseTypeValue::U64(v as u64),
                                (DwAte(5), 1) => BaseTypeValue::I8(v as i8),
                                (DwAte(5), 2) => BaseTypeValue::I16(v as i16),
                                (DwAte(5), 4) => BaseTypeValue::I32(v as i32),
                                (DwAte(5), 8) => BaseTypeValue::I64(v as i64),
                                (DwAte(4), 4) => BaseTypeValue::F32(v as f32),
                                (DwAte(4), 8) => BaseTypeValue::F64(v as f64),
                                _=> BaseTypeValue::Generic(v),
                            };
                                                                                          //
                            Ok(EvaluatorValue::Value(
                                correct_value,
                                ValueInformation::new(None, vec![ValuePiece::Dwarf { value: Some(value) }]),
                            ))
                        },
                        _ =>  {

                            Ok(EvaluatorValue::Value(
                                parsed_value,
                                ValueInformation {
                                    raw: None,
                                    pieces: vec![ValuePiece::Dwarf { value: Some(value) }],
                                },
                            ))
                        },
                    };

                }

                Location::Bytes { value } => {
                    // Remove piece if whole object is used.
                    let bit_size = 8 * (byte_size - all_bytes.len() as u64);
                    if pieces[0].should_remove(bit_size) {
                        pieces.remove(0);
                    }

                    return Ok(EvaluatorValue::Bytes(value.clone()));
                }
                Location::ImplicitPointer {
                    value: _,
                    byte_offset: _,
                } => {
                    error!("Unimplemented");
                    return Err(anyhow!("Unimplemented"));
                }
            }
        }

        while all_bytes.len() > byte_size as usize {
            all_bytes.pop(); // NOTE: Removes extra bytes if value is from register and less the 4 byts
        }

        Ok(EvaluatorValue::Value(
            BaseTypeValue::parse_base_type(all_bytes.clone(), encoding)?,
            ValueInformation::new(Some(all_bytes.clone()), value_pieces),
        ))
    }

    /// Evaluate and parse the type by going down the tree of type dies.
    ///
    /// Description:
    ///
    /// * `registers` - A register struct for accessing the register values.
    /// * `mem` - A struct for accessing the memory of the debug target.
    /// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
    /// * `unit` - A compilation unit which contains the given DIE.
    /// * `die` - The current type die in the type tree.
    /// * `data_offset` - The memory address offset.
    /// * `pieces` - A list of pieces containing the location and size information.
    fn eval_type<M: MemoryAccess>(
        registers: &Registers,
        mem: &mut M,
        dwarf: &gimli::Dwarf<R>,
        unit: &gimli::Unit<R>,
        die: &gimli::DebuggingInformationEntry<'_, '_, R>,
        data_offset: u64,
        pieces: &mut Vec<MyPiece<R>>,
    ) -> Result<EvaluatorValue<R>> {
        info!("tag: {:?}", die.tag());
        match die.tag() {
            gimli::DW_TAG_base_type => {
                // Make sure that the die has the tag DW_TAG_base_type.
                match die.tag() {
                    gimli::DW_TAG_base_type => (),
                    _ => {
                        error!("Expected DW_TAG_base_type die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_base_type die, this should never happen"
                        ));
                    }
                };

                check_alignment(die, data_offset, pieces)?;

                // Get byte size and encoding from the die.
                let byte_size = match attributes::byte_size_attribute(die)? {
                    Some(val) => val,
                    None => {
                        error!("Missing required byte size attribute");
                        return Err(anyhow!("Missing required byte size attribute"));
                    }
                };

                if byte_size == 0 {
                    return Ok(EvaluatorValue::ZeroSize);
                }

                let encoding = match attributes::encoding_attribute(die)? {
                    Some(val) => val,
                    None => {
                        error!("Missing required encoding attribute");
                        return Err(anyhow!("Missing required encoding attribute"));
                    }
                };

                // Evaluate the value.
                EvaluatorValue::handle_eval_piece(
                    registers,
                    mem,
                    byte_size,
                    data_offset, // TODO
                    encoding,
                    pieces,
                )
            }
            gimli::DW_TAG_pointer_type => {
                // Make sure that the die has the tag DW_TAG_pointer_type.
                match die.tag() {
                    gimli::DW_TAG_pointer_type => (),
                    _ => {
                        error!("Expected DW_TAG_pointer_type die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_pointer_type die, this should never happen"
                        ));
                    }
                };

                check_alignment(die, data_offset, pieces)?;

                // Get the name of the pointer type.
                let name = attributes::name_attribute(dwarf, die)?;

                // Evaluate the pointer type value.
                let address_class = match attributes::address_class_attribute(die)? {
                    Some(val) => val,
                    None => {
                        error!("Die is missing required attribute DW_AT_address_class");
                        return Err(anyhow!(
                            "Die is missing required attribute DW_AT_address_class"
                        ));
                    }
                };

                // This vill evaluate the address
                let address = match address_class.0 {
                    0 => {
                        EvaluatorValue::handle_eval_piece(
                            registers,
                            mem,
                            4, // This Should be set dependent on the system(4 for 32 bit systems)
                            data_offset,
                            DwAte(1),
                            pieces,
                        )?
                    }
                    _ => {
                        error!("Unimplemented DwAddr code"); // NOTE: The codes are architecture specific.
                        return Err(anyhow!("Unimplemented DwAddr code"));
                    }
                };

                let value = match (attributes::type_attribute(dwarf, unit, die)?, &address) {
                    (
                        Some((section_offset, unit_offset)),
                        EvaluatorValue::Value(BaseTypeValue::Address32(address_value), _),
                    ) => {
                        // Get the variable die.
                        let header = dwarf.debug_info.header_from_offset(
                            match section_offset.as_debug_info_offset() {
                                Some(val) => val,
                                None => {
                                    error!(
                                        "Could not convert section offset into debug info offset"
                                    );
                                    return Err(anyhow!(
                                        "Could not convert section offset into debug info offset"
                                    ));
                                }
                            },
                        )?;

                        let type_unit = gimli::Unit::new(dwarf, header)?;
                        let type_die = unit.entry(unit_offset)?;
                        let mut new_pieces = vec![MyPiece::new(Piece {
                            size_in_bits: None,
                            bit_offset: None,
                            location: Location::<R>::Address {
                                address: *address_value as u64,
                            },
                        })];
                        EvaluatorValue::eval_type(
                            registers,
                            mem,
                            dwarf,
                            &type_unit,
                            &type_die,
                            0,
                            &mut new_pieces,
                        )?
                    }
                    _ => EvaluatorValue::OptimizedOut,
                };

                return Ok(EvaluatorValue::PointerTypeValue(Box::new(
                    PointerTypeValue {
                        name,
                        address,
                        value,
                    },
                )));

                // TODO: Use DW_AT_type and the evaluated address to evaluate the pointer.
            }
            gimli::DW_TAG_array_type => {
                // Make sure that the die has the tag DW_TAG_array_type.
                match die.tag() {
                    gimli::DW_TAG_array_type => (),
                    _ => {
                        error!("Expected DW_TAG_array_type die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_array_type die, this should never happen"
                        ));
                    }
                };

                check_alignment(die, data_offset, pieces)?;

                let mut children = get_children(unit, die)?;
                let mut i = 0;
                while i < children.len() {
                    let die = unit.entry(children[i])?;
                    match die.tag() {
                        gimli::DW_TAG_subrange_type => (),
                        _ => {
                            let _c = children.remove(i);
                            i -= 1;
                        }
                    }
                    i += 1;
                }

                if children.len() != 1 {
                    error!("Unreachable");
                    return Err(anyhow!("Unreachable"));
                }

                let dimension_die = unit.entry(children[0])?;

                let subrange_type_value = match EvaluatorValue::eval_type(
                    registers,
                    mem,
                    dwarf,
                    unit,
                    &dimension_die,
                    data_offset,
                    pieces,
                )? {
                    EvaluatorValue::SubrangeTypeValue(subrange_type_value) => subrange_type_value,
                    _ => {
                        error!("Unreachable");
                        return Err(anyhow!("Unreachable"));
                    }
                };

                let mut values = vec![];

                // Evaluate all the values in the array.
                match subrange_type_value.get_count()? {
                    Some(count) => {
                        // Get type attribute unit and die.
                        let (type_unit, die_offset) = get_type_info(dwarf, unit, die)?;
                        let type_die = &type_unit.entry(die_offset)?;

                        // Evaluate all the values in the array.
                        for _i in 0..count {
                            values.push(EvaluatorValue::eval_type(
                                registers,
                                mem,
                                dwarf,
                                &type_unit,
                                type_die,
                                data_offset,
                                pieces,
                            )?);
                        }
                    }
                    None => (),
                };

                Ok(EvaluatorValue::Array(Box::new(ArrayTypeValue {
                    subrange_type_value,
                    values,
                })))
            }
            gimli::DW_TAG_structure_type => {
                // Make sure that the die has the tag DW_TAG_structure_type.
                match die.tag() {
                    gimli::DW_TAG_structure_type => (),
                    _ => {
                        error!("Expected DW_TAG_structure_type die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_structure_type die, this should never happen"
                        ));
                    }
                };

                check_alignment(die, data_offset, pieces)?;

                let name = match attributes::name_attribute(dwarf, die)? {
                    Some(val) => val,
                    None => {
                        error!("Expected the structure type die to have a name attribute");
                        return Err(anyhow!(
                            "Expected the structure type die to have a name attribute"
                        ));
                    }
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
                            )?];

                            return Ok(EvaluatorValue::Struct(Box::new(StructureTypeValue {
                                name,
                                members,
                            })));
                        }
                        gimli::DW_TAG_member => {
                            let data_member_location =
                                match attributes::data_member_location_attribute(&c_die)? {
                                    Some(val) => val,
                                    None => {
                                        error!(
                                "Expected member die to have attribute DW_AT_data_member_location"
                            );
                                        return Err(
                                            anyhow!(
                                "Expected member die to have attribute DW_AT_data_member_location"),
                                        );
                                    }
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
                        )?,
                        tag => {
                            error!("Unexpected die tag: {:?}", tag);
                            return Err(anyhow!("Unimplemented"));
                        }
                    };
                    members.push(member);
                }

                return Ok(EvaluatorValue::Struct(Box::new(StructureTypeValue {
                    name,
                    members,
                })));
            }
            gimli::DW_TAG_union_type => {
                // Make sure that the die has the tag DW_TAG_union_type.
                match die.tag() {
                    gimli::DW_TAG_union_type => (),
                    _ => {
                        error!("Expected DW_TAG_union_type die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_union_type die, this should never happen"
                        ));
                    }
                };

                check_alignment(die, data_offset, pieces)?;

                let name = match attributes::name_attribute(dwarf, die)? {
                    Some(val) => val,
                    None => {
                        error!("Expected union type die to have a name attribute");
                        return Err(anyhow!("Expected union type die to have a name attribute"));
                    }
                };

                // Get all children of type DW_TAG_member.
                let children = get_children(unit, die)?;
                let mut member_dies = vec![];
                for c in children {
                    let c_die = unit.entry(c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_member => {
                            let data_member_location =
                                match attributes::data_member_location_attribute(&c_die)? {
                                    Some(val) => val,
                                    None => {
                                        error!("Expected member die to have attribute DW_AT_data_member_location");
                                        return Err(anyhow!("Expected member die to have attribute DW_AT_data_member_location"));
                                    }
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
                        )?,
                        tag => {
                            error!("Unexpected die with tag {:?}", tag);
                            return Err(anyhow!("Unimplemented"));
                        }
                    };
                    members.push(member);
                }

                return Ok(EvaluatorValue::Union(Box::new(UnionTypeValue {
                    name,
                    members,
                })));
            }
            gimli::DW_TAG_member => {
                // Make sure that the die has the tag DW_TAG_member
                match die.tag() {
                    gimli::DW_TAG_member => (),
                    _ => {
                        error!("Expected DW_TAG_member die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_member die, this should never happen"
                        ));
                    }
                };

                // Get the name of the member.
                let name = attributes::name_attribute(dwarf, die)?;

                // Calculate the new data offset.
                let new_data_offset = match attributes::data_member_location_attribute(die)? {
                    // NOTE: Seams it can also be a location description and not an offset. Dwarf 5 page 118
                    Some(val) => data_offset + val,
                    None => data_offset,
                };

                check_alignment(die, new_data_offset, pieces)?;

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
                    _ => {
                        error!("Expected DW_TAG_enumeration_type die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_enumeration_type die, this should never happen"
                        ));
                    }
                };

                check_alignment(die, data_offset, pieces)?;

                // Get type attribute unit and die.
                let (type_unit, die_offset) = get_type_info(dwarf, unit, die)?;
                let type_die = &type_unit.entry(die_offset)?;

                // Get type value.
                let variant = EvaluatorValue::eval_type(
                    registers,
                    mem,
                    dwarf,
                    &type_unit,
                    type_die,
                    data_offset,
                    pieces,
                )?;

                // Go through the children and find the correct enumerator value.
                let children = get_children(unit, die)?;

                let mut enumerators = vec![];
                for c in children {
                    let c_die = unit.entry(c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_enumerator => {
                            let name = attributes::name_attribute(dwarf, &c_die)?;

                            let const_value = match attributes::const_value_attribute(&c_die)? {
                                Some(val) => val,
                                None => {
                                    error!("Expected enumeration type die to have attribute DW_AT_const_value");
                                    return Err(anyhow!("Expected enumeration type die to have attribute DW_AT_const_value"));
                                }
                            };

                            enumerators.push(EnumeratorValue { name, const_value });
                        }
                        gimli::DW_TAG_subprogram => (),
                        tag => {
                            error!("Unimplemented for tag: {:?}", tag);
                            return Err(anyhow!("Unimplemented"));
                        }
                    };
                }

                // Get the name of the enum type and the enum variant.
                let name = match attributes::name_attribute(dwarf, die)? {
                    Some(val) => val,
                    None => {
                        error!("Expected enumeration type die to have attribute DW_AT_name");
                        return Err(anyhow!(
                            "Expected enumeration type die to have attribute DW_AT_name"
                        ));
                    }
                };

                Ok(EvaluatorValue::Enum(Box::new(EnumerationTypeValue {
                    name,
                    variant,
                    enumerators,
                })))
            }
            gimli::DW_TAG_variant_part => {
                // Make sure that the die has tag DW_TAG_variant_part
                match die.tag() {
                    gimli::DW_TAG_variant_part => (),
                    _ => {
                        error!("Expected DW_TAG_variant_part die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_variant_part die, this should never happen"
                        ));
                    }
                };

                check_alignment(die, data_offset, pieces)?;

                // Get the enum variant.
                // TODO: If variant is optimised out then return optimised out and remove the pieces for
                // this type if needed.

                let variant: Option<MemberValue<R>> = match attributes::discr_attribute(die)? {
                    Some(die_offset) => {
                        let member_die = &unit.entry(die_offset)?;

                        // Evaluate the DW_TAG_member value.
                        match member_die.tag() {
                            gimli::DW_TAG_member => match EvaluatorValue::eval_type(
                                registers,
                                mem,
                                dwarf,
                                unit,
                                member_die,
                                data_offset,
                                pieces,
                            )? {
                                EvaluatorValue::Member(member) => Some(*member),
                                _ => {
                                    error!("Unreachable");
                                    return Err(anyhow!("Unreachable"));
                                }
                            },
                            _ => {
                                error!("Unreachable");
                                return Err(anyhow!("Unreachable"));
                            }
                        }
                    }
                    None => None,
                };

                // The value should be a unsigned int thus convert the value to a u64.
                let variant_number = match variant.clone() {
                    Some(MemberValue { name: _name, value }) => match value.to_value() {
                        Some(val) => Some(get_udata(val)?),
                        None => None,
                    },
                    None => None,
                };

                let original_pieces = pieces.clone();
                // Find  all the DW_TAG_variant dies and evaluate them.
                let mut variants = vec![];
                let children = get_children(unit, die)?;
                for c in &children {
                    let c_die = unit.entry(*c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_variant => {
                            let mut temp_pieces = original_pieces.clone();
                            // Evaluate the value of the variant.
                            let variant = match EvaluatorValue::eval_type(
                                registers,
                                mem,
                                dwarf,
                                unit,
                                &c_die,
                                data_offset,
                                &mut temp_pieces,
                            )? {
                                EvaluatorValue::VariantValue(variant) => variant,
                                _ => {
                                    error!("Unreachable");
                                    return Err(anyhow!("Unreachable"));
                                }
                            };

                            match (variant.discr_value, variant_number) {
                                (Some(discr_value), Some(variant_num)) => {
                                    // If This is the variant then update the piece index.
                                    if discr_value == variant_num {
                                        *pieces = temp_pieces;
                                    }
                                }
                                _ => (),
                            };

                            variants.push(*variant);
                        }
                        _ => (),
                    };
                }

                Ok(EvaluatorValue::VariantPartValue(Box::new(
                    VariantPartValue { variant, variants },
                )))
            }
            gimli::DW_TAG_variant => {
                check_alignment(die, data_offset, pieces)?;

                let mut members = vec![];

                // Find the child die of type DW_TAG_member
                let children = get_children(unit, die)?;
                for c in children {
                    let c_die = unit.entry(c)?;
                    match c_die.tag() {
                        gimli::DW_TAG_member => {
                            // Evaluate the value of the member.
                            let member = match EvaluatorValue::eval_type(
                                registers,
                                mem,
                                dwarf,
                                unit,
                                &c_die,
                                data_offset,
                                pieces,
                            )? {
                                EvaluatorValue::Member(member) => member,
                                _ => {
                                    error!("Unreachable");
                                    return Err(anyhow!("Unreachable"));
                                }
                            };

                            members.push(member);
                        }
                        _ => (),
                    };
                }

                if members.len() != 1 {
                    error!("Unreachable");
                    return Err(anyhow!("Unreachable"));
                    // DW_TAG_variant should only have one member child.
                }

                let discr_value = attributes::discr_value_attribute(die)?;

                Ok(EvaluatorValue::VariantValue(Box::new(VariantValue {
                    discr_value,
                    child: *members[0].clone(),
                })))
            }
            gimli::DW_TAG_subrange_type => {
                // Make sure that the die has the tag DW_TAG_subrange_type
                match die.tag() {
                    gimli::DW_TAG_subrange_type => (),
                    _ => {
                        error!("Expected DW_TAG_subrange_type die, this should never happen");
                        return Err(anyhow!(
                            "Expected DW_TAG_subrange_type die, this should never happen"
                        ));
                    }
                };

                let lower_bound = attributes::lower_bound_attribute(die)?;

                // If the die has a count attribute then that is the value.
                match attributes::count_attribute(die)? {
                    // NOTE: This could be replace with lower and upper bound
                    Some(count) => Ok(EvaluatorValue::SubrangeTypeValue(SubrangeTypeValue {
                        lower_bound,
                        count: Some(count),
                        base_type_value: None,
                    })),
                    None => {
                        // Get the type unit and die.
                        let (type_unit, die_offset) = match get_type_info(dwarf, unit, die) {
                            Ok(val) => val,
                            Err(_) => {
                                error!("Expected subrange type die to have type information");
                                return Err(anyhow!(
                                    "Expected subrange type die to have type information"
                                ));
                            }
                        };
                        let type_die = &type_unit.entry(die_offset)?;

                        // Evaluate the type attribute value.
                        let base_type_value = match EvaluatorValue::eval_type(
                            registers,
                            mem,
                            dwarf,
                            &type_unit,
                            type_die,
                            data_offset,
                            pieces,
                        )? {
                            EvaluatorValue::Value(base_type_value, value_information) => {
                                Some((base_type_value, value_information))
                            }
                            _ => {
                                error!("Unreachable");
                                return Err(anyhow!("Unreachable"));
                            }
                        };
                        Ok(EvaluatorValue::SubrangeTypeValue(SubrangeTypeValue {
                            lower_bound,
                            count: None,
                            base_type_value,
                        }))
                    }
                }
            }
            gimli::DW_TAG_subroutine_type => {
                error!("Unimplemented");
                Err(anyhow!("Unimplemented"))
            }
            gimli::DW_TAG_subprogram => {
                error!("Unimplemented");
                Err(anyhow!("Unimplemented"))
            }
            gimli::DW_TAG_string_type => {
                error!("Unimplemented");
                Err(anyhow!("Unimplemented"))
            }
            gimli::DW_TAG_generic_subrange => {
                error!("Unimplemented");
                Err(anyhow!("Unimplemented"))
            }
            gimli::DW_TAG_template_type_parameter => {
                error!("Unimplemented");
                Err(anyhow!("Unimplemented"))
            }
            tag => {
                error!("Unimplemented for tag {:?}", tag);
                Err(anyhow!("Unimplemented"))
            }
        }
    }
}

/// Parse a `BaseTypeValue` struct to a `u64` value.
///
/// Description:
///
/// * `value` - The `BaseTypeValue` that will be turned into a `u64`.
pub fn get_udata(value: BaseTypeValue) -> Result<u64> {
    match value {
        BaseTypeValue::U8(v) => Ok(v as u64),
        BaseTypeValue::U16(v) => Ok(v as u64),
        BaseTypeValue::U32(v) => Ok(v as u64),
        BaseTypeValue::U64(v) => Ok(v),
        BaseTypeValue::Generic(v) => Ok(v),
        _ => {
            error!("Unimplemented");
            Err(anyhow!("Unimplemented"))
        }
    }
}

/// Format a `Vec` of `EvaluatorValue`s into a `String` that describes the value and type.
///
/// Description:
///
/// * `values` - A list of `EvaluatorValue`s that will be formatted into a `String`.
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

/// Format a `Vec` of `EvaluatorValue`s into a `String` that describes the type.
///
/// Description:
///
/// * `values` - A list of `EvaluatorValue`s that will be formatted into a `String`.
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

/// Struct that represents a array type.
#[derive(Debug, Clone)]
pub struct ArrayTypeValue<R: Reader<Offset = usize>> {
    /// subrange_type information.
    pub subrange_type_value: SubrangeTypeValue,

    /// The list of values in the array.
    pub values: Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for ArrayTypeValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[ {} ]", format_values(&self.values))
    }
}

impl<R: Reader<Offset = usize>> ArrayTypeValue<R> {
    /// Get the type of the array as a `String`.
    pub fn get_type(&self) -> String {
        format!("[ {} ]", format_types(&self.values))
    }
}

/// Struct that represents a struct type.
#[derive(Debug, Clone)]
pub struct StructureTypeValue<R: Reader<Offset = usize>> {
    /// The name of the struct.
    pub name: String,

    /// All the attributes of the struct.
    pub members: Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for StructureTypeValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {{ {} }}", self.name, format_values(&self.members))
    }
}

impl<R: Reader<Offset = usize>> StructureTypeValue<R> {
    /// Get the type of the struct as a `String`.
    pub fn get_type(&self) -> String {
        format!("{} {{ {} }}", self.name, format_types(&self.members))
    }
}

/// Struct that represents a enum type.
#[derive(Debug, Clone)]
pub struct EnumerationTypeValue<R: Reader<Offset = usize>> {
    /// The name of the Enum.
    pub name: String,

    /// The name of the Enum.
    pub variant: EvaluatorValue<R>,

    /// The value of the enum.
    pub enumerators: Vec<EnumeratorValue>,
}

impl<R: Reader<Offset = usize>> fmt::Display for EnumerationTypeValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}", self.name, self.variant)
    }
}

impl<R: Reader<Offset = usize>> EnumerationTypeValue<R> {
    /// Get the type of the enum as a `String`.
    pub fn get_type(&self) -> String {
        format!("{}::{}", self.name, self.variant.get_type())
    }
}

/// Struct that represents a union type.
#[derive(Debug, Clone)]
pub struct UnionTypeValue<R: Reader<Offset = usize>> {
    /// The name of the union type
    pub name: String,

    /// The values of the union type.
    pub members: Vec<EvaluatorValue<R>>,
}

impl<R: Reader<Offset = usize>> fmt::Display for UnionTypeValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ( {} )", self.name, format_values(&self.members))
    }
}

impl<R: Reader<Offset = usize>> UnionTypeValue<R> {
    /// Get the type of the union as a `String`.
    pub fn get_type(&self) -> String {
        format!("{} ( {} )", self.name, format_types(&self.members))
    }
}

/// Struct that represents a attribute type.
#[derive(Debug, Clone)]
pub struct MemberValue<R: Reader<Offset = usize>> {
    /// The name of the attribute.
    pub name: Option<String>,

    /// The value of the attribute.
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
    /// Get the type of the attribute as a `String`.
    pub fn get_type(&self) -> String {
        match &self.name {
            Some(name) => format!("{}::{}", name, self.value.get_type()),
            None => format!("{}", self.value.get_type()),
        }
    }
}

/// Struct that represents a pointer type.
#[derive(Debug, Clone)]
pub struct PointerTypeValue<R: Reader<Offset = usize>> {
    /// The name of the pointer type.
    pub name: Option<String>,

    /// The value of the attribute.
    pub address: EvaluatorValue<R>,

    /// The value stored at the pointed location
    pub value: EvaluatorValue<R>,
    // DW_TAG_pointer_type contains:
    // * DW_AT_type
    // * DW_AT_name
    // * DW_AT_address_class
}

impl<R: Reader<Offset = usize>> fmt::Display for PointerTypeValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match &self.name {
            Some(name) => write!(f, "{}::{}", name, self.value),
            None => write!(f, "{}", self.value),
        };
    }
}

impl<R: Reader<Offset = usize>> PointerTypeValue<R> {
    /// Get the type of the pointer type as a `String`.
    pub fn get_type(&self) -> String {
        match &self.name {
            Some(name) => format!("{}::{}", name, self.value.get_type()),
            None => format!("{}", self.value.get_type()),
        }
    }
}

/// Struct that represents a enumerator.
#[derive(Debug, Clone)]
pub struct EnumeratorValue {
    /// The name of the enumerator.
    pub name: Option<String>,

    /// The value of the attribute.
    pub const_value: u64,
    // DW_TAG_enumerator contains:
    // * DW_AT_name
    // * DW_AT_const_value
}

impl fmt::Display for EnumeratorValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match &self.name {
            Some(name) => write!(f, "{}::{}", name, self.const_value),
            None => write!(f, "{}", self.const_value),
        };
    }
}

impl EnumeratorValue {
    /// Get the type of the enumerator as a `String`.
    pub fn get_type(&self) -> String {
        format!("{:?}", self.name)
    }
}

/// Struct that represents a variant.
#[derive(Debug, Clone)]
pub struct VariantValue<R: Reader<Offset = usize>> {
    /// The discr value
    pub discr_value: Option<u64>,

    /// The child value
    pub child: MemberValue<R>,
    // DW_TAG_variant contains:
    // * DW_AT_discr_value
    // * A child with tag DW_TAG_member
}

impl<R: Reader<Offset = usize>> fmt::Display for VariantValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match &self.discr_value {
            Some(discr) => write!(f, "{}::{}", discr, self.child),
            None => write!(f, "{}", self.child),
        };
    }
}

impl<R: Reader<Offset = usize>> VariantValue<R> {
    /// Get the type of the variant as a `String`.
    pub fn get_type(&self) -> String {
        match &self.discr_value {
            Some(discr) => format!("{} {}", discr, self.child.get_type()),
            None => format!("{}", self.child.get_type()),
        }
    }
}

/// Struct that represents a variant_part.
#[derive(Debug, Clone)]
pub struct VariantPartValue<R: Reader<Offset = usize>> {
    /// The variant value
    pub variant: Option<MemberValue<R>>,

    /// The variants
    pub variants: Vec<VariantValue<R>>,
    // DW_TAG_variant_part contains:
    // * DW_AT_discr_value
    // * A child with tag DW_TAG_member
    // * Children with tag DW_TAG_variant
}

impl<R: Reader<Offset = usize>> fmt::Display for VariantPartValue<R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut variants = "{".to_string();
        for v in &self.variants {
            variants = format!("{} {},", variants, v);
        }
        variants = format!("{} {}", variants, "}");
        return match &self.variant {
            // TODO: Improve
            Some(variant) => write!(f, "< variant: {} >, {}", variant, variants),
            None => write!(f, "{}", variants),
        };
    }
}

impl<R: Reader<Offset = usize>> VariantPartValue<R> {
    /// Get the type of the variant_part as a `String`.
    pub fn get_type(&self) -> String {
        // TODO: Improve
        match &self.variant {
            Some(variant) => format!("{}", variant),
            None => format!("",),
        }
    }
}

/// Struct that represents a variant.
#[derive(Debug, Clone)]
pub struct SubrangeTypeValue {
    /// The lowser bound
    pub lower_bound: Option<u64>,

    /// The count
    pub count: Option<u64>,

    /// The count value but evaluated. // TODO: Combine count and number to one attriute.
    pub base_type_value: Option<(BaseTypeValue, ValueInformation)>,
    // DW_TAG_variant contains:
    // * DW_AT_type
    // * DW_AT_lower_bound
    // * DW_AT_count
}

impl fmt::Display for SubrangeTypeValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self.get_count() {
            Ok(Some(count)) => write!(f, "{}", count),
            _ => write!(f, ""),
        };
    }
}

impl SubrangeTypeValue {
    /// Get the type of the subrange_type as a `String`.
    pub fn get_type(&self) -> String {
        match &self.base_type_value {
            Some((val, _)) => format!("{}", val.get_type()),
            None => format!("u64"),
        }
    }

    pub fn get_count(&self) -> Result<Option<u64>> {
        match self.count {
            Some(val) => Ok(Some(val)),
            None => match &self.base_type_value {
                Some((btv, _)) => Ok(Some(get_udata(btv.clone())?)),
                None => Ok(None),
            },
        }
    }
}

/// A enum representing the base types in DWARF.
#[derive(Debug, Clone)]
pub enum BaseTypeValue {
    /// generic value.
    Generic(u64),

    /// 32 bit address.
    Address32(u32),

    /// 32 bit register value.
    Reg32(u32),

    /// boolean
    Bool(bool),

    /// 8 bit unsigned integer.
    U8(u8),

    /// 16 bit unsigned integer.
    U16(u16),

    /// 32 bit unsigned integer.
    U32(u32),

    /// 64 bit unsigned integer.
    U64(u64),

    /// 8 bit signed integer.
    I8(i8),

    /// 16 bit signed integer.
    I16(i16),

    /// 32 bit signed integer.
    I32(i32),

    /// 64 bit signed integer.
    I64(i64),

    /// 32 bit float.
    F32(f32),

    /// 64 bit float.
    F64(f64),
}

impl fmt::Display for BaseTypeValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return match self {
            BaseTypeValue::Bool(val) => write!(f, "{}", val),
            BaseTypeValue::Generic(val) => write!(f, "{}", val),
            BaseTypeValue::I8(val) => write!(f, "{}", val),
            BaseTypeValue::U8(val) => write!(f, "{}", val),
            BaseTypeValue::I16(val) => write!(f, "{}", val),
            BaseTypeValue::U16(val) => write!(f, "{}", val),
            BaseTypeValue::I32(val) => write!(f, "{}", val),
            BaseTypeValue::U32(val) => write!(f, "{}", val),
            BaseTypeValue::I64(val) => write!(f, "{}", val),
            BaseTypeValue::U64(val) => write!(f, "{}", val),
            BaseTypeValue::F32(val) => write!(f, "{}", val),
            BaseTypeValue::F64(val) => write!(f, "{}", val),
            BaseTypeValue::Address32(val) => write!(f, "'Address' {:#10x}", val),
            BaseTypeValue::Reg32(val) => write!(f, "0x{:x}", val),
        };
    }
}

impl BaseTypeValue {
    /// Parse a DWARF base type.
    ///
    /// Description:
    ///
    /// * `data` - The value in bytes.
    /// * `encoding` - The DWARF encoding of the value.
    ///
    /// Will parse the given bytes into the encoding type.
    /// The size of the given `data` parameter will be used when parsing.
    pub fn parse_base_type(data: Vec<u8>, encoding: DwAte) -> Result<BaseTypeValue> {
        if data.len() == 0 {
            return Err(anyhow!("Expected data to be larger then 0"));
        }

        // TODO: Fix so not any data size can be sent into this function.
        Ok(match (encoding, data.len()) {
            // Source: DWARF 4 page 168-169 and 77
            (DwAte(1), 4) => BaseTypeValue::Address32(u32::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // DW_ATE_address = 1 // TODO: Different size addresses?
            (DwAte(2), 1) => BaseTypeValue::Bool(
                (u8::from_le_bytes(match data.try_into() {
                    Ok(val) => val,
                    Err(err) => {
                        error!("{:?}", err);
                        return Err(anyhow!("{:?}", err));
                    }
                })) == 1,
            ), // DW_ATE_boolean = 2 // TODO: Use modulus?
            (DwAte(2), 2) => BaseTypeValue::Bool(
                (u16::from_le_bytes(match data.try_into() {
                    Ok(val) => val,
                    Err(err) => {
                        error!("{:?}", err);
                        return Err(anyhow!("{:?}", err));
                    }
                })) == 1,
            ), // DW_ATE_boolean = 2 // TODO: Use modulus?
            (DwAte(2), 4) => BaseTypeValue::Bool(
                (u32::from_le_bytes(match data.try_into() {
                    Ok(val) => val,
                    Err(err) => {
                        error!("{:?}", err);
                        return Err(anyhow!("{:?}", err));
                    }
                })) == 1,
            ), // DW_ATE_boolean = 2 // TODO: Use modulus?

            //        (DwAte(3), _) => ,   // DW_ATE_complex_float = 3 // NOTE: Seems like a C++ thing
            (DwAte(4), 4) => BaseTypeValue::F32(f32::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // DW_ATE_float = 4
            (DwAte(4), 8) => BaseTypeValue::F64(f64::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // DW_ATE_float = 4

            (DwAte(5), 1) => BaseTypeValue::I8(i8::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_signed = 5, 8)
            (DwAte(5), 2) => BaseTypeValue::I16(i16::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_signed = 5, 16)
            (DwAte(5), 4) => BaseTypeValue::I32(i32::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_signed = 5, 32)
            (DwAte(5), 8) => BaseTypeValue::I64(i64::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_signed = 5, 64)

            //        (DwAte(6), _) => ,     // DW_ATE_signed_char = 6 // TODO: Add type
            (DwAte(7), 1) => BaseTypeValue::U8(u8::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_unsigned = 7, 8)
            (DwAte(7), 2) => BaseTypeValue::U16(u16::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_unsigned = 7, 16)
            (DwAte(7), 4) => BaseTypeValue::U32(u32::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_unsigned = 7, 32)
            (DwAte(7), 8) => BaseTypeValue::U64(u64::from_le_bytes(match data.try_into() {
                Ok(val) => val,
                Err(err) => {
                    error!("{:?}", err);
                    return Err(anyhow!("{:?}", err));
                }
            })), // (DW_ATE_unsigned = 7, 64)
            _ => {
                error!("encoding {}, byte_size: {}", encoding, data.len());
                return Err(anyhow!("encoding {}, byte_size: {}", encoding, data.len()));
            }
        })
    }

    /// Get the base type as a `String` with the Rust names.
    pub fn get_type(&self) -> String {
        match self {
            BaseTypeValue::Bool(_) => "bool".to_owned(),
            BaseTypeValue::Generic(_) => "<unknown>".to_owned(),
            BaseTypeValue::I8(_) => "i8".to_owned(),
            BaseTypeValue::U8(_) => "u8".to_owned(),
            BaseTypeValue::I16(_) => "i16".to_owned(),
            BaseTypeValue::U16(_) => "u16".to_owned(),
            BaseTypeValue::I32(_) => "i32".to_owned(),
            BaseTypeValue::U32(_) => "u32".to_owned(),
            BaseTypeValue::I64(_) => "i64".to_owned(),
            BaseTypeValue::U64(_) => "u64".to_owned(),
            BaseTypeValue::F32(_) => "f32".to_owned(),
            BaseTypeValue::F64(_) => "f63".to_owned(),
            BaseTypeValue::Address32(_) => "<32 bit address>".to_owned(),
            BaseTypeValue::Reg32(_) => "<32 bit register value>".to_owned(),
        }
    }
}

/// Convert a `BaseTypeValue` to a `gimli::Value`.
///
/// Description:
///
/// * `value` - The value that will be converted into a `gimli::Value` stuct.
pub fn convert_to_gimli_value(value: BaseTypeValue) -> gimli::Value {
    match value {
        BaseTypeValue::Bool(val) => gimli::Value::Generic(match val {
            true => 1,
            false => 0,
        }),
        BaseTypeValue::Generic(val) => gimli::Value::Generic(val),
        BaseTypeValue::I8(val) => gimli::Value::I8(val),
        BaseTypeValue::U8(val) => gimli::Value::U8(val),
        BaseTypeValue::I16(val) => gimli::Value::I16(val),
        BaseTypeValue::U16(val) => gimli::Value::U16(val),
        BaseTypeValue::I32(val) => gimli::Value::I32(val),
        BaseTypeValue::U32(val) => gimli::Value::U32(val),
        BaseTypeValue::I64(val) => gimli::Value::I64(val),
        BaseTypeValue::U64(val) => gimli::Value::U64(val),
        BaseTypeValue::F32(val) => gimli::Value::F32(val),
        BaseTypeValue::F64(val) => gimli::Value::F64(val),
        BaseTypeValue::Address32(val) => gimli::Value::Generic(val as u64),
        BaseTypeValue::Reg32(val) => gimli::Value::U32(val),
    }
}

/// Convert a `gimli::Value` to a `BaseTypeValue`.
///
/// Description:
///
/// * `value` - The value that will be converted into a `BaseTypeValue` stuct.
pub fn convert_from_gimli_value(value: gimli::Value) -> BaseTypeValue {
    match value {
        gimli::Value::Generic(val) => BaseTypeValue::Generic(val),
        gimli::Value::I8(val) => BaseTypeValue::I8(val),
        gimli::Value::U8(val) => BaseTypeValue::U8(val),
        gimli::Value::I16(val) => BaseTypeValue::I16(val),
        gimli::Value::U16(val) => BaseTypeValue::U16(val),
        gimli::Value::I32(val) => BaseTypeValue::I32(val),
        gimli::Value::U32(val) => BaseTypeValue::U32(val),
        gimli::Value::I64(val) => BaseTypeValue::I64(val),
        gimli::Value::U64(val) => BaseTypeValue::U64(val),
        gimli::Value::F32(val) => BaseTypeValue::F32(val),
        gimli::Value::F64(val) => BaseTypeValue::F64(val),
    }
}

/// Will retrieve the type DIE and compilation unit for a given die.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `unit` - A compilation unit which contains the given DIE.
/// * `die` - The DIE which contain a reference to the type DIE.
fn get_type_info<R: Reader<Offset = usize>>(
    dwarf: &gimli::Dwarf<R>,
    unit: &gimli::Unit<R>,
    die: &gimli::DebuggingInformationEntry<'_, '_, R>,
) -> Result<(gimli::Unit<R>, gimli::UnitOffset)> {
    let (unit_offset, die_offset) = match attributes::type_attribute(dwarf, unit, die)? {
        Some(val) => val,
        None => {
            error!("Die doesn't have the required DW_AT_type attribute");
            return Err(anyhow!(
                "Die doesn't have the required DW_AT_type attribute"
            ));
        }
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
                None => {
                    error!("Could not get unit from unit offset");
                    return Err(anyhow!("Could not get unit from unit offset"));
                }
            }
        }
    };

    Ok((unit, die_offset))
}

/// Will check that the address is correctly aligned.
///
/// Description:
///
/// * `die` - The type DIE to check alignment for.
/// * `data_offset` - The memory address offset.
/// * `pieces` - A list of pieces containing the location and size information.
fn check_alignment<R: Reader<Offset = usize>>(
    die: &gimli::DebuggingInformationEntry<'_, '_, R>,
    mut data_offset: u64,
    pieces: &Vec<MyPiece<R>>,
) -> Result<()> {
    match attributes::alignment_attribute(die)? {
        Some(alignment) => {
            if pieces.len() == 0 {
                return Ok(());
            }

            if pieces.len() < 1 {
                data_offset = 0;
            }

            match pieces[0].piece.location {
                Location::Address { address } => {
                    let mut addr = address + (data_offset / 4) * 4;
                    addr -= addr % 4; // TODO: Is this correct?

                    if addr % alignment != 0 {
                        error!("Address not aligned");
                        return Err(anyhow!("Address not aligned"));
                    }
                }
                _ => (),
            };
        }
        None => (),
    };

    Ok(())
}

/// Will retrieve the list of children DIEs for a DIE.
///
/// Description:
///
/// * `unit` - The compilation unit which contains the given DIE.
/// * `die` - The DIE to find the children for.
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

/// Will remove the unnecessary bytes.
///
/// Description:
///
/// * `bytes` - The bytes to be trimmed of unnecessary bytes.
/// * `piece` - The piece the given bytes is evaluated from.
/// * `byte_size` - The byte size of the resulting trim.
///
/// Some pieces contain more bytes then the type describes.
/// Thus this function removes those unused bytes.
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
            //    error!("Expected the offset to be in bytes, got {} bits", offset);
            //    return Err(anyhow!("Expected the offset to be in bytes, got {} bits", offset));
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

/// Contains the unparsed value and the location of it.
#[derive(Debug, Clone)]
pub struct ValueInformation {
    pub raw: Option<Vec<u8>>, // byte size and raw value
    pub pieces: Vec<ValuePiece>,
}

impl ValueInformation {
    /// Create a new `ValueInformation` struct
    ///
    /// Description:
    ///
    /// * `raw` - The unparsed value.
    /// * `pieces` - The location of the value.
    pub fn new(raw: Option<Vec<u8>>, pieces: Vec<ValuePiece>) -> ValueInformation {
        ValueInformation { raw, pieces }
    }
}

/// A struct that describes the size and location of a value.
#[derive(Debug, Clone)]
pub enum ValuePiece {
    /// Contains which register the value is located and the size of it.
    Register {
        /// The register the value is stored.
        register: u16,

        /// The size of the value.
        byte_size: usize,
    },

    /// Contains which address the value is located and the size of it.
    Memory {
        /// The address the value is stored.
        address: u32,

        /// The size of the value.
        byte_size: usize,
    },

    /// Contains the value stored on the DWARF stack.
    Dwarf {
        /// The value stored on the DWARF stack.
        /// If it is `None` then the value is optimized out.
        value: Option<gimli::Value>,
    },
}
