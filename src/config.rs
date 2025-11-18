use serde::Deserialize;
use std::{fs, io, path::Path};

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

pub fn setup_config() -> io::Result<Config> {
    let config_dir_path = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?
        .join(CONFIG_DIR_NAME);

    // Create directory if it doesn't exist
    fs::create_dir_all(&config_dir_path)?;

    let config_path = config_dir_path.join(CONFIG_FILE_NAME);

    // Write default config if file doesn't exist
    if !config_path.exists() {
        fs::write(
            &config_path,
            "# Configuration for sqledger\n\n# PostgreSQL connection string.\ndatabase_url = \"postgresql://postgres:postgres@localhost/postgres\"\n",
        )?;
    }

    Ok(load_config(&config_path))
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

