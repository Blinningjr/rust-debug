use anyhow::{anyhow, Result};
use log::error;

use crate::variable::is_variable_die;

use gimli::{ColumnType, DebuggingInformationEntry, Dwarf, Reader, Unit};

use crate::utils::{
    get_current_unit, get_debug_info_header, get_unit_and_die_offset_from_attribute,
};

use std::num::NonZeroU64;

/// Contains all the information about where the code was declared in the source code.
#[derive(Debug, Clone)]
pub struct SourceInformation {
    /// The source code directory where the debug information was declared.
    pub directory: Option<String>,

    /// The relative source code file path where the debug information was declared.
    pub file: Option<String>,

    /// The source code line number where the debug information was declared.
    pub line: Option<NonZeroU64>,

    /// The source code column number where the debug information was declared.
    pub column: Option<NonZeroU64>,
}

impl SourceInformation {
    /// Retrieves the information about where the given DIE was declared in the source code.
    ///
    /// Description:
    ///
    /// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
    /// * `unit` - A reference to gimli-rs `Unit` struct, which the given DIE is located in.
    /// * `die` - A reference to the DIE containing attributes starting with `DW_AT_decl_`.
    /// * `cwd` - The work directory of the debugged program.
    ///
    ///This function will retrieve the information stored in the attributes starting with
    ///`DW_AT_decl_` from the given DIE>
    pub fn get_die_source_information<R: Reader<Offset = usize>>(
        dwarf: &Dwarf<R>,
        unit: &Unit<R>,
        die: &DebuggingInformationEntry<'_, '_, R>,
        cwd: &str,
    ) -> Result<SourceInformation> {
        let (file, directory) = match die.attr_value(gimli::DW_AT_decl_file)? {
            Some(gimli::AttributeValue::FileIndex(v)) => match &unit.line_program {
                Some(lp) => {
                    let header = lp.header();
                    match header.file(v) {
                        Some(file_entry) => {
                            let (file, directory) = match file_entry.directory(header) {
                                Some(dir_av) => {
                                    let mut dir_raw =
                                        dwarf.attr_string(unit, dir_av)?.to_string()?.to_string();
                                    let file_raw = dwarf
                                        .attr_string(unit, file_entry.path_name())?
                                        .to_string()?
                                        .to_string();
                                    let file = file_raw.trim_start_matches(&dir_raw).to_string();

                                    if !dir_raw.starts_with('/') {
                                        dir_raw = format!("{}/{}", cwd, dir_raw);
                                    }

                                    (file, Some(dir_raw))
                                }
                                None => (
                                    dwarf
                                        .attr_string(unit, file_entry.path_name())?
                                        .to_string()?
                                        .to_string(),
                                    None,
                                ),
                            };

                            (Some(file), directory)
                        }
                        None => (None, None),
                    }
                }
                None => (None, None),
            },
            None => (None, None),
            Some(v) => {
                error!("Unimplemented {:?}", v);
                return Err(anyhow!("Unimplemented {:?}", v));
            }
        };

        let line = match die.attr_value(gimli::DW_AT_decl_line)? {
            Some(gimli::AttributeValue::Udata(v)) => NonZeroU64::new(v),
            None => None,
            Some(v) => {
                error!("Unimplemented {:?}", v);
                return Err(anyhow!("Unimplemented {:?}", v));
            }
        };

        let column = match die.attr_value(gimli::DW_AT_decl_column)? {
            Some(gimli::AttributeValue::Udata(v)) => NonZeroU64::new(v),
            None => None,
            Some(v) => {
                error!("Unimplemented {:?}", v);
                return Err(anyhow!("Unimplemented {:?}", v));
            }
        };

        Ok(SourceInformation {
            directory,
            file,
            line,
            column,
        })
    }

    pub fn get_from_address<R: Reader<Offset = usize>>(
        dwarf: &Dwarf<R>,
        address: u64,
        cwd: &str,
    ) -> Result<SourceInformation> {
        let unit = get_current_unit(dwarf, address as u32)?;
        let mut nearest = None;
        match unit.line_program.clone() {
            Some(line_program) => {
                let (program, sequences) = line_program.sequences()?;
                let mut in_range_seqs = vec![];
                for seq in sequences {
                    if address >= seq.start && address < seq.end {
                        in_range_seqs.push(seq);
                    }
                }
                //           println!("number of seqs: {:?}", in_range_seqs.len());
                //           println!("pc: {:?}", address);
                let mut result = vec![];
                //let mut all = 0;
                for seq in in_range_seqs {
                    let mut sm = program.resume_from(&seq);
                    while let Some((header, row)) = sm.next_row()? {
                        //                   println!(
                        //                        "address: {:?}, line: {:?}, is_stmt: {:?}, valid: {:?}",
                        //                       row.address(),
                        //                       row.line(),
                        //                       row.is_stmt(),
                        //                       row.address() == address
                        //                   );

                        if row.address() <= address {
                            let (file, directory) = match row.file(header) {
                                Some(file_entry) => match file_entry.directory(header) {
                                    Some(dir_av) => {
                                        let mut dir_raw = dwarf
                                            .attr_string(&unit, dir_av)?
                                            .to_string()?
                                            .to_string();
                                        let file_raw = dwarf
                                            .attr_string(&unit, file_entry.path_name())?
                                            .to_string()?
                                            .to_string();
                                        let file =
                                            file_raw.trim_start_matches(&dir_raw).to_string();

                                        if !dir_raw.starts_with('/') {
                                            dir_raw = format!("{}/{}", cwd, dir_raw);
                                        }

                                        (Some(file), Some(dir_raw))
                                    }
                                    None => (None, None),
                                },
                                None => (None, None),
                            };

                            let si = SourceInformation {
                                directory,
                                file,
                                line: row.line(),
                                column: match row.column() {
                                    ColumnType::LeftEdge => NonZeroU64::new(1),
                                    ColumnType::Column(n) => Some(n),
                                },
                            };

                            match nearest {
                                Some((addr, _)) => {
                                    if row.address() > addr {
                                        nearest = Some((row.address(), si));
                                    }
                                }
                                None => nearest = Some((row.address(), si)),
                            };
                        }
                        if row.address() == address {
                            result.push(row.line());
                        }
                        //                        all += 1;
                    }
                }
                //println!("total line rows: {:?}", all);
                //           println!("result line rows: {:?}", result.len());
                match nearest {
                    Some((_, si)) => Ok(si),
                    None => {
                        error!("Could not find source informaitno");
                        Err(anyhow!("Could not find source informaitno"))
                    }
                }
            }
            None => {
                error!("Unit has no line program");
                Err(anyhow!("Unit has no line program"))
            }
        }
    }

    /// Retrieve the variables source location where it was declared.
    ///
    /// Description:
    ///
    /// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
    /// * `unit` - A reference to gimli-rs `Unit` struct, which the given DIE is located in.
    /// * `die` - A reference to DIE.
    /// * `cwd` - The work directory of the debugged program.
    ///
    /// This function will retrieve the source code location where the variable was declared.
    /// The information is retrieved from the  attributes starting with `DW_AT_decl_` in the given DIE,
    /// or in the DIE found in the attribute `DW_AT_abstract_origin`.
    pub fn find_variable_source_information<R: Reader<Offset = usize>>(
        dwarf: &Dwarf<R>,
        unit: &Unit<R>,
        die: &DebuggingInformationEntry<R>,
        cwd: &str,
    ) -> Result<SourceInformation> {
        if is_variable_die(die) {
            return Err(anyhow!("This die is not a variable"));
        }

        if let Ok(Some(attribute)) = die.attr_value(gimli::DW_AT_abstract_origin) {
            let (section_offset, unit_offset) =
                get_unit_and_die_offset_from_attribute(dwarf, unit, attribute)?;

            let header = get_debug_info_header(dwarf, &section_offset)?;
            let abstract_unit = gimli::Unit::new(dwarf, header)?;
            let abstract_die = unit.entry(unit_offset)?;

            return Self::find_variable_source_information(
                dwarf,
                &abstract_unit,
                &abstract_die,
                cwd,
            );
        }

        Self::get_die_source_information(dwarf, unit, die, cwd)
    }
}

/// Find the machine code address that corresponds to a line in the source file.
///
/// Description:
///
/// * `dwarf` - A reference to gimli-rs `Dwarf` struct.
/// * `cwd` - The work directory of the debugged program.
/// * `path` - The relative path to the source file from the work directory of the debugged program.
/// * `line` - A line number in the source program.
/// * `column` - A optional column number in the source program.
///
/// Finds the machine code address that is generated from the given source code file and line
/// number.
/// If there are multiple machine codes for that line number it takes the first one and the one.
// Good source: DWARF section 6.2
pub fn find_breakpoint_location<'a, R: Reader<Offset = usize>>(
    dwarf: &'a Dwarf<R>,
    cwd: &str,
    path: &str,
    line: NonZeroU64,
    column: Option<NonZeroU64>,
) -> Result<Option<u64>> {
    let mut locations = vec![];

    let mut units = dwarf.units();
    while let Some(unit_header) = units.next()? {
        let unit = dwarf.unit(unit_header)?;

        if let Some(ref line_program) = unit.line_program {
            let lp_header = line_program.header();

            for file_entry in lp_header.file_names() {
                let directory = match file_entry.directory(lp_header) {
                    Some(dir_av) => {
                        let dir_raw = dwarf.attr_string(&unit, dir_av)?;
                        dir_raw.to_string()?.to_string()
                    }
                    None => continue,
                };

                let file_raw = dwarf.attr_string(&unit, file_entry.path_name())?;
                let mut file_path = format!("{}/{}", directory, file_raw.to_string()?);

                if !file_path.starts_with('/') {
                    // TODO: Find a better solution
                    file_path = format!("{}/{}", cwd, file_path);
                }

                if path == file_path {
                    let mut rows = line_program.clone().rows();
                    while let Some((header, row)) = rows.next_row()? {
                        let file_entry = match row.file(header) {
                            Some(v) => v,
                            None => continue,
                        };

                        let directory = match file_entry.directory(header) {
                            Some(dir_av) => {
                                let dir_raw = dwarf.attr_string(&unit, dir_av)?;
                                dir_raw.to_string()?.to_string()
                            }
                            None => continue,
                        };

                        let file_raw = dwarf.attr_string(&unit, file_entry.path_name())?;
                        let mut file_path = format!("{}/{}", directory, file_raw.to_string()?);
                        if !file_path.starts_with('/') {
                            // TODO: Find a better solution
                            file_path = format!("{}/{}", cwd, file_path);
                        }

                        if path == file_path {
                            if let Some(l) = row.line() {
                                if line == l {
                                    locations.push((row.column(), row.address()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    match locations.len() {
        0 => Ok(None),
        len => {
            let search = match column {
                Some(v) => gimli::ColumnType::Column(v),
                None => gimli::ColumnType::LeftEdge,
            };

            let mut res = locations[0];
            for location in locations.iter().take(len).skip(1) {
                if location.0 <= search && location.0 > res.0 {
                    res = *location;
                }
            }

            Ok(Some(res.1))
        }
    }
}
