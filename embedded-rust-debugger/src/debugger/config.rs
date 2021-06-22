use std::path::PathBuf;

pub struct Config {
    pub binary:     Option<PathBuf>,
    pub chip:       Option<String>,
    pub cwd:        Option<String>,
    pub probe_num:  usize,
}

impl Config {
    pub fn new(opt: Option<super::Opt>) -> Config {
        let binary = match opt {
            Some(val) => val.file_path,
            None => None,
        };
        Config {
            binary: binary,
            chip: Some("STM32F411RETx".to_owned()), // TODO:
            cwd: None,
            probe_num: 0,
        }
    }

    pub fn is_missing_config(&self) -> bool {
        self.binary.is_none() || self.chip.is_none() || self.cwd.is_none()
    }

    pub fn missing_config_message(&self) -> String {
        if !self.is_missing_config() {
            return "No required configurations missing".to_owned();
        }

        let mut error = "Missing required configurations:".to_owned();
        if self.binary.is_none() {
            error = format!("{}\n\t{}", error, "binary file");
        }
        if self.chip.is_none() {
            error = format!("{}\n\t{}", error, "chip");
        }
        if self.cwd.is_none() {
            error = format!("{}\n\t{}", error, "cwd");
        }

        error
    }
}

