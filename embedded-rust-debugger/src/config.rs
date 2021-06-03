use std::path::PathBuf;

pub struct Config {
    pub bin:        Option<PathBuf>,
    pub chip:       Option<String>,
    pub probe_num:  u32,
}

impl Config {
    pub fn new(opt: super::Opt) -> Config {
        Config {
            bin: opt.file_path,
            chip: None,
            probe_num: 0,
        }
    }

    pub fn is_missing_config(&self) -> bool {
        self.bin.is_none() || self.chip.is_none()
    }

    pub fn missing_config(&self) -> String {
        if !self.is_missing_config() {
            return "No required configurations missing".to_owned();
        }

        let mut error = "Missing required configurations:".to_owned();
        if self.bin.is_none() {
            error = format!("{}\n\t{}", error, "binary file");
        }
        if self.chip.is_none() {
            error = format!("{}\n\t{}", error, "chip");
        }

        error
    }

}

