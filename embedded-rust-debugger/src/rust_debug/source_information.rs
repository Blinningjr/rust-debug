use anyhow::Result;

use gimli::{
    Reader,
    Dwarf,
    Unit,
    DebuggingInformationEntry,
};


#[derive(Debug, Clone)]
pub struct SourceInformation {
    pub directory:  Option<String>,
    pub file:       Option<String>,
    pub line:       Option<u64>,
    pub column:     Option<u64>,
}


impl SourceInformation {

    pub fn get_die_source_information<R: Reader<Offset = usize>>(dwarf: & Dwarf<R>,
                                                                 unit:   &Unit<R>,
                                                                 die:    &DebuggingInformationEntry<'_, '_, R>,
                                                                 cwd:    &str
                                                                ) -> Result<SourceInformation>
    {
        let (file, directory) = match die.attr_value(gimli::DW_AT_decl_file)? {
            Some(gimli::AttributeValue::FileIndex(v)) => {
                match &unit.line_program {
                    Some(lp) => {
                        let header = lp.header();
                        match header.file(v) {
                            Some(file_entry)    => {
                                let (file, directory) = match file_entry.directory(header) {
                                    Some(dir_av) => {
                                        let mut dir_raw = dwarf.attr_string(&unit, dir_av)?.to_string()?.to_string();
                                        let file_raw = dwarf.attr_string(&unit, file_entry.path_name())?.to_string()?.to_string();
                                        let file = file_raw.trim_start_matches(&dir_raw).to_string();
    
                                        if !dir_raw.starts_with("/") {
                                            dir_raw = format!("{}/{}", cwd, dir_raw);
                                        }
    
                                        (file, Some(dir_raw)) 
                                    },
                                    None => (dwarf.attr_string(&unit, file_entry.path_name())?.to_string()?.to_string(), None),
                                };
    
                                (Some(file), directory)
                            },
                            None        => (None, None),
                        }
                    },
                    None    => (None, None),
                }
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
            directory: directory,
            file: file,
            line: line,
            column: column,
        })
    }
}

