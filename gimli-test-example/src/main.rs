//! A simple example of parsing `.debug_info`.

use object::{Object, ObjectSection};
use std::{borrow, env, fs};


fn main() {
    for path in env::args().skip(1) {
        let file = fs::File::open(&path).unwrap();
        let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };
        let object = object::File::parse(&*mmap).unwrap();
        let endian = if object.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };
        dump_file(&object, endian).unwrap();
    }
}

fn dump_file(object: &object::File, endian: gimli::RunTimeEndian) -> Result<(), gimli::Error> {
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
            // If we find an entry for a function, print it.
            if entry.tag() == gimli::DW_TAG_subprogram {
                println!("Found a function: {:?}", entry);
                return Ok(());
            }
        }
    }

    Ok(())
}

