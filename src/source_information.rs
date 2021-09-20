use anyhow::Result;

use gimli::{DebuggingInformationEntry, Dwarf, Reader, Unit};

#[derive(Debug, Clone)]
pub struct SourceInformation {
    pub directory: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub column: Option<u64>,
}

impl SourceInformation {
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
