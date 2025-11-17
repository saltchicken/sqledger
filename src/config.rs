use serde::Deserialize;
use std::{fs, path::Path};

pub const CONFIG_DIR_NAME: &str = "sqledger";
pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Deserialize, Debug)]
pub struct Config {

    #[serde(default = "default_database_url")]
    pub database_url: String,
}

fn default_database_url() -> String {
    "postgresql://postgres:postgres@localhost/postgres".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: default_database_url(),
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