//! Testing how to parse and evaluate dwarf information.


use std::{borrow, env, fs};
use std::path::Path;

use probe_rs::{
    Probe,
    Core,
    MemoryInterface,
};
use probe_rs::flashing::{
    Format,
    download_file,
};
use core::time::Duration;

use object::{Object, ObjectSection};

use gimli::{
    EndianSlice,
    RunTimeEndian,
    RangeIter,
    Unit,
    Dwarf,
    Error,
    DebuggingInformationEntry,
    EvaluationResult::{
        Complete,
        RequiresMemory,
        RequiresRegister,
        RequiresFrameBase,
        RequiresTls,
        RequiresCallFrameCfa,
        RequiresAtLocation,
        RequiresEntryValue,
        RequiresParameterRef,
        RequiresRelocatedAddress,
        RequiresIndexedAddress,
        RequiresBaseType,
    },
    AttributeValue::{
        DebugStrRef,
        UnitRef,
        DebugLineRef,
        RangeListsRef,
        Udata,
        Encoding,
    },
    Attribute,
    ReaderOffset,
    Reader,
    EntriesTreeNode,
    Evaluation,
    EvaluationResult,
    UnitOffset,
    Register,
    Value,
    DwAte,
    Expression,
    Piece,
    Location,
    DieReference,
};

struct Debugger<'a, R: Reader<Offset = usize>> {
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

    pub fn process_tree(&mut self, 
            mut node: EntriesTreeNode<R>,
            prev_in_range: bool,
            mut frame_base: Option<u64>
        ) -> gimli::Result<bool>
    {
        let die = node.entry();
        let in_range = die_in_range(&self.dwarf, &self.unit, die, self.pc);
        let mut in_r = true;
        match (in_range, prev_in_range) {
            (Some(false), _ ) => in_r = false, //return Ok(()),
            (None, false) => in_r = false, //return Ok(()),
            _ => (),
        };
        println!("in_r: {:?}", in_r);
        if let Some(fb) = self.check_die(die, frame_base) {
            frame_base = Some(fb);
        }
//        if die.tag() == gimli::DW_TAG_variable {
//            if let Some(name) =  die.attr_value(gimli::DW_AT_name)? {
//                if let DebugStrRef(offset) = name  {
//                    if dwarf.string(offset).unwrap().to_string().unwrap() == "my_num" {
//                        return Ok(true);
//                    }
//                }
//            }
//        }
        if in_r {
            let mut children = node.children();
            while let Some(child) = children.next()? {
                // Recursively process a child.
                if self.process_tree(child, in_r, frame_base)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn check_die(&mut self,
                     die: &DebuggingInformationEntry<'_, '_, R>,
                     mut frame_base: Option<u64>
        ) -> Option<u64>
    {
    
        let mut attrs = die.attrs();
        println!("{:?}", die.tag().static_string());
        println!(
            "{:<30} | {:<}",
            "Name", "Value"
        );
        println!("----------------------------------------------------------------");
        while let Some(attr) = attrs.next().unwrap() {
            let val = match attr.value() {
                DebugStrRef(offset) => format!("{:?}", self.dwarf.string(offset).unwrap().to_string().unwrap()),
                _ => format!("{:?}", attr.value()),
            };
    
            println!(
                "{: <30} | {:<?}",
                attr.name().static_string().unwrap(),
                val
            );
            if let Some(expr) = attr.value().exprloc_value() {
                if attr.name() == gimli::DW_AT_frame_base {
                    frame_base = match self.new_evaluate(&self.unit, expr, frame_base).unwrap() {
                        Value::U64(v) => Some(v),
                        Value::U32(v) => Some(v as u64),
                        _ => frame_base,
                    };
                } else {
                    self.new_evaluate(self.unit, expr, frame_base);
                }
            }
        }
        println!("\n");
    
        return frame_base;
    }

    pub fn new_evaluate(&mut self,
                        unit: &Unit<R>,
                        expr: Expression<R>,
                        frame_base: Option<u64>
                    ) -> Result<Value, &'static str>
    {
        let mut eval = expr.evaluation(self.unit.encoding());
        let mut result = eval.evaluate().unwrap();
    
        println!("fb: {:?}", frame_base);
        loop {
            println!("{:#?}", result);
            match result {
                Complete => break,
                RequiresMemory{address, size, space, base_type} =>
                    self.resolve_requires_mem(unit, &mut eval, &mut result, address, size, space, base_type),
                RequiresRegister{register, base_type} => self.resolve_requires_reg(unit, &mut eval, &mut result, register, base_type),
                RequiresFrameBase => 
                    result = eval.resume_with_frame_base(frame_base.unwrap()).unwrap(), // TODO: Check and test if correct.
                RequiresTls(_tls) => unimplemented!(), // TODO
                RequiresCallFrameCfa => unimplemented!(), // TODO
                RequiresAtLocation(die_ref) => self.resolve_requires_at_location(unit, &mut eval, &mut result, frame_base, die_ref)?,
                RequiresEntryValue(e) =>
                  result = eval.resume_with_entry_value(self.new_evaluate(unit, e, frame_base)?).unwrap(),
                RequiresParameterRef(unit_offset) => //unimplemented!(), // TODO: Check and test if correct.
                    {
                        let die = unit.entry(unit_offset).unwrap();
                        let expr = die.attr_value(gimli::DW_AT_call_value).unwrap().unwrap().exprloc_value().unwrap();
                        let value = self.new_evaluate(unit, expr, frame_base).unwrap();
                        if let Value::U64(val) = value {
                            result = eval.resume_with_parameter_ref(val).unwrap();
                        } else {
                            return Err("could not find parameter");
                        }
                    },
                RequiresRelocatedAddress(num) =>
                    result = eval.resume_with_relocated_address(num).unwrap(), // TODO: Check and test if correct.
                RequiresIndexedAddress {index, relocate} => //unimplemented!(), // TODO: Check and test if correct. Also handle rolocate flag
                    result = eval.resume_with_indexed_address(self.dwarf.address(unit, index).unwrap()).unwrap(),
                RequiresBaseType(unit_offset) => 
                    result = eval.resume_with_base_type(
                        parse_base_type(unit, 0, unit_offset).value_type()).unwrap(),
            };
        }
    
        let value = self.eval_pieces(eval.result());
        println!("Value: {:?}", value);
        value
    }


    fn eval_pieces(&mut self,
                   pieces: Vec<Piece<R>>
                   ) -> Result<Value, &'static str>
    {
        // TODO: What should happen if more then one piece is given?
        if pieces.len() > 1 {
            panic!("Found more then one piece");
        }
        println!("{:?}", pieces);
        return self.eval_piece(&pieces[0]);
    }
    
    fn eval_piece(&mut self,
                  piece: &Piece<R>
                  ) -> Result<Value, &'static str>
    {
        // TODO: Handle size_in_bits and bit_offset
        match &piece.location {
            Location::Empty => return Err("Optimized out"),
            Location::Register { register } => // TODO Always return U32?
                return Ok(Value::U32(self.core.read_core_reg(register.0).unwrap())),
            Location::Address { address } =>  // TODO Always return U32?
                return Ok(Value::U32(self.core.read_word_32(*address as u32).map_err(|e| "Read error")?)),
            Location::Value { value } => return Ok(value.clone()),
            Location::Bytes { value } => unimplemented!(), // TODO
            Location::ImplicitPointer { value, byte_offset } => unimplemented!(), // TODO
        };
    }

    /*
     * Resolves requires memory when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_mem(&mut self,
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            address: u64,
            size: u8,
            _space: Option<u64>,
            base_type: UnitOffset<usize>
        )
            where R: Reader<Offset = usize>
    {
        let data = self.core.read_word_32(address as u32).unwrap();
        let value = parse_base_type(unit, data, base_type);
        *result = eval.resume_with_memory(value).unwrap();    
    }


    /*
     * Resolves requires register when evaluating a die.
     * TODO: Check and test if correct.
     */
    fn resolve_requires_reg(&mut self,
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            reg: Register,
            base_type: UnitOffset<usize>
        ) 
            where R: Reader<Offset = usize>
    {
        let data = self.core.read_core_reg(reg.0).unwrap();
        let value = parse_base_type(unit, data, base_type);
        *result = eval.resume_with_register(value).unwrap();    
    }

    fn resolve_requires_at_location(&mut self,
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            frame_base: Option<u64>,
            die_ref: DieReference<usize>
        ) -> Result<(), &'static str>
            where R: Reader<Offset = usize>
    { 
        match die_ref {
            DieReference::UnitRef(unit_offset) => {
                return self.help_at_location(unit, eval, result, frame_base, unit_offset);
            },
            DieReference::DebugInfoRef(debug_info_offset) => {
                let unit_header = self.dwarf.debug_info.header_from_offset(debug_info_offset).map_err(|_| "Can't find debug info header")?;
                if let Some(unit_offset) = debug_info_offset.to_unit_offset(&unit_header) {
                    let new_unit = self.dwarf.unit(unit_header).map_err(|_| "Can't find unit using unit header")?;
                    return self.help_at_location(&new_unit, eval, result, frame_base, unit_offset);
                } else {
                    return Err("Could not find at location");
                }    
            },
        };
    }


    fn help_at_location(&mut self,
            unit: &Unit<R>,
            eval: &mut Evaluation<R>,
            result: &mut EvaluationResult<R>,
            frame_base: Option<u64>,
            unit_offset: UnitOffset<usize>
        ) -> Result<(), &'static str>
            where R: Reader<Offset = usize>
    {
        let die = unit.entry(unit_offset).unwrap();
        if let Some(expr) = die.attr_value(gimli::DW_AT_location).unwrap().unwrap().exprloc_value() {
    
            let val = self.new_evaluate(&unit, expr, frame_base);
            unimplemented!(); // TODO: Add a value enum
    //          eval.resume_with_at_location(val.bytes); // val need to be of type bytes: R
        }
        else {
            return Err("die has no at location");
        }
    }

}

fn main() {
    probe_rs_stuff().unwrap();
}


fn probe_rs_stuff() -> Result<(), &'static str> {
    // Get a list of all available debug probes.
    let probes = Probe::list_all();
    
    // Use the first probe found.
    let probe = probes[0].open().map_err(|_| "Failed to open probe")?;
    
    // Attach to a chip.
    let mut session = probe.attach_under_reset("STM32F411RETx").map_err(|_| "Failed to attach probe to target")?;


//    println!("{:#?}", core.registers().PC());
//    println!("{:#?}", core.registers().program_counter().address);
    let path_str = env::args().skip(1).next().unwrap();
    let path = Path::new(&path_str);
    println!("{:#?}", path);
    
    download_file(&mut session, &path, Format::Elf).map_err(|_| "Failed to flash target");
    
    let mut core = session.core(0).unwrap();

    core.reset().map_err(|_| "Faild to reset")?;

    core.wait_for_core_halted(Duration::new(5, 0)).map_err(|_| "Core never halted");

    let pc_value: u32 = core
        .read_core_reg(core.registers().program_counter())
        .unwrap();

    println!("{:#02x}", pc_value);

    read_dwarf(pc_value, core, path);
    Ok(())
}


fn read_dwarf(pc: u32, core: Core, path: &Path) {
    let file = fs::File::open(&path).unwrap();
    let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };
    let object = object::File::parse(&*mmap).unwrap();
    let endian = if object.is_little_endian() {
        gimli::RunTimeEndian::Little
    } else {
        gimli::RunTimeEndian::Big
    };
    dump_file(object, endian, pc, core).unwrap();
}


fn dump_file(object: object::File, endian: gimli::RunTimeEndian, pc: u32, core: Core) -> Result<(), gimli::Error> {
    // Load a section and return as `Cow<[u8]>`.
    let loader = |id: gimli::SectionId| -> Result<borrow::Cow<[u8]>, gimli::Error> {
        match object.section_by_name(id.name()) {
            Some(ref section) => Ok(section
                .uncompressed_data()
                .unwrap_or(borrow::Cow::Borrowed(&[][..]))),
            None => Ok(borrow::Cow::Borrowed(&[][..])),
        }
    };

    // Load a supplementary section. We don't have a supplementary object file,
    // so always return an empty slice.
    let sup_loader = |_| Ok(borrow::Cow::Borrowed(&[][..]));

    // Load all of the sections.
    let dwarf_cow = gimli::Dwarf::load(&loader, &sup_loader)?;

    // Borrow a `Cow<[u8]>` to create an `EndianSlice`.
    let borrow_section: &dyn for<'b> Fn(
        &'b borrow::Cow<[u8]>,
    ) -> gimli::EndianSlice<'b, gimli::RunTimeEndian> =
        &|section| gimli::EndianSlice::new(&*section, endian);

    // Create `EndianSlice`s for all of the sections.
    let dwarf = dwarf_cow.borrow(&borrow_section);


    let unit = get_current_unit(&dwarf, pc)?;
    println!("{:?}", unit.name.unwrap().to_string());

//    let dies = get_current_dies(&dwarf, &unit, pc)?;
//    println!("{:#?}", dies.iter().map(|d| d.tag().static_string()).collect::<Vec<_>>());
//
//    for die in dies.iter() {
//        check_die(&dwarf, &unit, die, pc);
//    }

    let mut tree = unit.entries_tree(None)?;
    let root = tree.root()?;

    let mut debugger = Debugger::new(core, dwarf, &unit, pc);
    debugger.process_tree(root, false, None)?; 
    

    return Ok(());
}


fn get_current_unit<'a, R>(
        dwarf: &'a Dwarf<R>,
        pc: u32
    ) -> Result<Unit<R>, Error>
        where R: Reader<Offset = usize>
{
    // TODO: Maybe return a Vec of units

    let mut iter = dwarf.units();
    while let Some(header) = iter.next()? {
        let unit = dwarf.unit(header)?;
        if Some(true) == in_range(pc, &mut dwarf.unit_ranges(&unit).unwrap()) {
            return Ok(unit);
        }
    }
    return Err(Error::MissingUnitDie);
}


fn get_current_dies<'a, R>(
        dwarf: &'a Dwarf<R>,
        unit: &'a Unit<R>,
        pc: u32
    ) -> Result<Vec<DebuggingInformationEntry<'a, 'a, R>>, Error>
        where R: Reader<Offset = usize>
{
    let mut entries = unit.entries();
    let mut dies: Vec<DebuggingInformationEntry<R>> = vec!();
    while let Some((_, entry)) = entries.next_dfs()? {
//        println!("{:#?}", entry.tag().static_string());
        if Some(true) == in_range(pc, &mut dwarf.die_ranges(unit, entry)?) {
            dies.push(entry.clone());
        }
    }
    return Ok(dies);
}


fn in_range<R>(pc: u32, rang: &mut RangeIter<R>) -> Option<bool>
        where R: Reader<Offset = usize>
{ 
    let mut no_range = true;
    while let Ok(Some(range)) = rang.next() {
//        println!("range: {:?}", range);
        if range.begin <= pc as u64 && range.end >= pc as u64 {
            return Some(true);
        }
        no_range = false;
    }
    if no_range {
        return None;
    }
    return Some(false);
}


fn die_in_range<'a, R>(
        dwarf: &'a Dwarf<R>,
        unit: &'a Unit<R>,
        die: &DebuggingInformationEntry<'_, '_, R>,
        pc: u32,)
    -> Option<bool>
        where R: Reader<Offset = usize>
{
    match dwarf.die_ranges(unit, die) {
        Ok(mut range) => in_range(pc, &mut range),
        Err(_) => None,
    }
}

fn parse_base_type<R>(
        unit: &Unit<R>,
        data: u32,
        base_type: UnitOffset<usize>
    ) -> Value
        where R: Reader<Offset = usize>
{
    if base_type.0 == 0 {
        return Value::Generic(data as u64);
    }
    let die = unit.entry(base_type).unwrap();

    // I think that the die returned must be a base type tag.
    if die.tag() != gimli::DW_TAG_base_type {
        println!("{:?}", die.tag().static_string());
        panic!("die tag not base type");
    }

    let encoding = match die.attr_value(gimli::DW_AT_encoding) {
        Ok(Some(Encoding(dwate))) => dwate,
        _ => panic!("expected Encoding"),
    };
    let byte_size = match die.attr_value(gimli::DW_AT_byte_size) {
        Ok(Some(Udata(v))) => v,
        _ => panic!("expected Udata"),
    };
    
    // Check dwarf doc for the codes.
    match (encoding, byte_size) {
        (DwAte(7), 1) => Value::U8(data as u8),     // (unsigned, 8)
        (DwAte(7), 2) => Value::U16(data as u16),   // (unsigned, 16)
        (DwAte(7), 4) => Value::U32(data as u32),   // (unsigned, 32)
        (DwAte(7), 8) => Value::U64(data as u64),   // (unsigned, 64) TODO: Fix
        
        (DwAte(5), 1) => Value::I8(data as i8),     // (signed, 8)
        (DwAte(5), 2) => Value::I16(data as i16),   // (signed, 16)
        (DwAte(5), 4) => Value::I32(data as i32),   // (signed, 32)
        (DwAte(5), 8) => Value::I64(data as i64),   // (signed, 64) TODO: Fix
        _ => unimplemented!(),
    }
}

