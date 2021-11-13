use anyhow::{anyhow, Result};

use crate::utils::get_current_unit;

use gimli::{DebuggingInformationEntry, Dwarf, Reader, Unit};

/// Contains all the information about where the code was declared in the source code.
#[derive(Debug, Clone)]
pub struct SourceInformation {
    /// The source code directory where the debug information was declared.
    pub directory: Option<String>,

    /// The relative source code file path where the debug information was declared.
    pub file: Option<String>,

    /// The source code line number where the debug information was declared.
    pub line: Option<u64>,

    /// The source code column number where the debug information was declared.
    pub column: Option<u64>,
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
                                        dwarf.attr_string(&unit, dir_av)?.to_string()?.to_string();
                                    let file_raw = dwarf
                                        .attr_string(&unit, file_entry.path_name())?
                                        .to_string()?
                                        .to_string();
                                    let file = file_raw.trim_start_matches(&dir_raw).to_string();

                                    if !dir_raw.starts_with("/") {
                                        dir_raw = format!("{}/{}", cwd, dir_raw);
                                    }

                                    (file, Some(dir_raw))
                                }
                                None => (
                                    dwarf
                                        .attr_string(&unit, file_entry.path_name())?
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
            Some(v) => unimplemented!("{:?}", v),
        };

        let line = match die.attr_value(gimli::DW_AT_decl_line)? {
            Some(gimli::AttributeValue::Udata(v)) => Some(v),
            None => None,
            Some(v) => unimplemented!("{:?}", v),
        };

        let column = match die.attr_value(gimli::DW_AT_decl_column)? {
            Some(gimli::AttributeValue::Udata(v)) => Some(v),
            None => None,
            Some(v) => unimplemented!("{:?}", v),
        };

        Ok(SourceInformation {
            directory,
            file,
            line,
            column,
        })
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
    line: u64,
    column: Option<u64>,
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
                let mut file_path = format!("{}/{}", directory, file_raw.to_string()?.to_string());

                if !file_path.starts_with("/") {
                    // TODO: Find a better solution
                    file_path = format!("{}/{}", cwd, file_path);
                }

                if path == &file_path {
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
                        let mut file_path =
                            format!("{}/{}", directory, file_raw.to_string()?.to_string());
                        if !file_path.starts_with("/") {
                            // TODO: Find a better solution
                            file_path = format!("{}/{}", cwd, file_path);
                        }

                        if path == &file_path {
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
        0 => return Ok(None),
        len => {
            let search = match column {
                Some(v) => gimli::ColumnType::Column(v),
                None => gimli::ColumnType::LeftEdge,
            };

            let mut res = locations[0];
            for i in 1..len {
                if locations[i].0 <= search && locations[i].0 > res.0 {
                    res = locations[i];
                }
            }

            return Ok(Some(res.1));
        }
    };
}



pub fn get_line_number<R: Reader<Offset = usize>>(
        dwarf: &Dwarf<R>,
        address: u64,
    ) -> Result<Option<u64>> {

    let unit = get_current_unit(dwarf, address as u32)?;
    match unit.line_program {
        Some(line_program) => {
            let (program, sequences) = line_program.sequences()?;
            let mut in_range_seqs = vec!();
            for seq in sequences {
                if address >= seq.start && address < seq.end {
                    in_range_seqs.push(seq);
                }
            }
            println!("number of seqs: {:?}", in_range_seqs.len());
            println!("pc: {:?}", address);
            let mut result = vec!();
            let mut all = 0;
            for seq in in_range_seqs {
                let mut sm = program.resume_from(&seq);
                while let Some((_lph, row)) = sm.next_row()? {
                    println!("address: {:?}, line: {:?}, is_stmt: {:?}, valid: {:?}", row.address(), row.line(), row.is_stmt(), row.address() == address);
                    if row.address() == address {
                        result.push(row.line());
                    }
                    all += 1;
                }
            }
            println!("total line rows: {:?}", all);
            println!("result line rows: {:?}", result.len());
            if result.len() < 1 {
                return Ok(None);
            }
            return Ok(result[0]);
        },
        None => Err(anyhow!("Unit has no line program")),
    }
}
