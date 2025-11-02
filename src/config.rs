use serde::Deserialize;
use std::{fs, path::Path};

pub const DB_NAME: &str = "scripts.db";
pub const CONFIG_DIR_NAME: &str = "sqledger";
pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const DEFAULT_SCRIPTS_DIR: &str = "~/.config/sqledger/scripts";

#[derive(Deserialize, Debug)]
pub struct Config {
    #[serde(default = "default_script_dir")]
    pub script_directory: String,
}

fn default_script_dir() -> String {
    DEFAULT_SCRIPTS_DIR.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            script_directory: default_script_dir(),
        }
    }
}

pub fn load_config(config_path: &Path) -> Config {
    if let Ok(content) = fs::read_to_string(config_path) {
        return toml::from_str(&content).unwrap_or_else(|e| {
            println!(
                "Failed to parse config file at {:?}: {}, using default.",
                config_path, e
            );
            Config::default()
        });
    }
    Config::default()
}
