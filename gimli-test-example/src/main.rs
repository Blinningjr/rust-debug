//! A simple example of parsing `.debug_info`.


use object::{Object, ObjectSection};
use std::{borrow, env, fs};
use probe_rs::{Probe, Core};

use gimli::{EndianSlice, Evaluation, EvaluationResult, Format, LittleEndian, Value};


fn main() {
    probe_rs_stuff().unwrap();
}


fn probe_rs_stuff() -> Result<(), &'static str> {
    // Get a list of all available debug probes.
    let probes = Probe::list_all();
    
    // Use the first probe found.
    let mut probe = probes[0].open().map_err(|_| "Failed to open probe")?;
    
    // Attach to a chip.
    let mut session = probe.attach_under_reset("STM32F411RETx").map_err(|_| "Failed to attach probe to target")?;

    let mut core = session.core(0).unwrap();

//    println!("{:#?}", core.registers().PC());
//    println!("{:#?}", core.registers().program_counter().address);

    let pc_value: u32 = core
        .read_core_reg(core.registers().program_counter())
        .unwrap();

    println!("{:#02x}", pc_value);

    read_dwarf(pc_value, &mut core);
    Ok(())
}


fn read_dwarf(pc: u32, core: &mut Core) {
    for path in env::args().skip(1) {
        let file = fs::File::open(&path).unwrap();
        let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };
        let object = object::File::parse(&*mmap).unwrap();
        let endian = if object.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };
        dump_file(&object, endian, pc, core).unwrap();
    }
}


fn dump_file(object: &object::File, endian: gimli::RunTimeEndian, pc: u32, core: &mut Core) -> Result<(), gimli::Error> {
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
    let borrow_section: &dyn for<'a> Fn(
        &'a borrow::Cow<[u8]>,
    ) -> gimli::EndianSlice<'a, gimli::RunTimeEndian> =
        &|section| gimli::EndianSlice::new(&*section, endian);

    // Create `EndianSlice`s for all of the sections.
    let dwarf = dwarf_cow.borrow(&borrow_section);

    // Iterate over all compilation units.
    let mut iter = dwarf.units();
    while let Some(header) = iter.next()? {
        // Parse the abbreviations and other information for this compilation unit.
        let unit = dwarf.unit(header)?;
    
        // Iterate over all of this compilation unit's entries.
        let mut entries = unit.entries();
        while let Some((_, entry)) = entries.next_dfs()? {
            if entry.tag() == gimli::DW_TAG_variable {
                let mut attrs = entry.attrs();
                println!(
                    "{:<30} | {:<}",
                    "Name", "Value"
                );
                println!("----------------------------------------------------------------");
                while let Some(attr) = attrs.next().unwrap() {
                   // println!(
                   //     "{: <30} | {:<?}",
                   //     attr.name().static_string().unwrap(),
                   //     attr.value()
                   // );

                    if attr.name() == gimli::DW_AT_location {
                        println!(
                            "{: <30} | {:<?}",
                            attr.name().static_string().unwrap(),
                            attr.value()
                        );
                        if let Some(expr) = attr.exprloc_value() {
                            let mut eval = expr.evaluation(unit.encoding());
                            let mut result = eval.evaluate().unwrap();
                            if result != EvaluationResult::Complete {
                                match result {
                                    EvaluationResult::RequiresRegister { register, base_type } => {
                                        let reg_num = match register { gimli::Register(v) => v, _ => panic!("err"),};
                                        let offset = match base_type { gimli::UnitOffset(v) => v, _ => panic!("err"),};
                                        println!("{:?} | {:?}", reg_num, offset);
                                        if let Ok(v) = core.read_core_reg(reg_num) {
                                            println!("{:#02x}", v);
                                            result = eval.resume_with_register(gimli::Value::U32(v)).unwrap();
                                        } else {
                                            panic!("Err");
                                        }

                                        return Ok(());
                                    },
                                    //EvaluationResult::RequiresFrameBase => {
                                    //    let frame_base = get_frame_base();
                                    //    result = eval.resume_with_frame_base(frame_base).unwrap();
                                    //},
                                    _ => (),
                                    //_ => unimplemented!(),
                                }; 
                            }
                            //let result = eval.result();
                            //println!("{:?}", result);
                        }
                    } 
                }

                println!("\n\n");
//                return Ok(());
            }
        }
    }

    Ok(())
}

