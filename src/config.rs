use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io, path::Path};

pub const CONFIG_DIR_NAME: &str = "sqledger";
pub const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    #[serde(default = "default_connections")]
    pub connections: HashMap<String, String>,
}


fn default_connections() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(
        "local".to_string(),
        "postgresql://postgres:postgres@localhost/postgres".to_string(),
    );
    map
}

impl Default for Config {
    fn default() -> Self {
        Self {
            connections: default_connections(),
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
            r#"# Configuration for sqledger

[connections]
local = "postgresql://postgres:postgres@localhost/postgres"
# production = "postgresql://user:password@remote_host/dbname"
"#,
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

pub fn save_config(connections: &HashMap<String, String>) -> io::Result<()> {
    let config_dir_path = dirs::config_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not find config directory"))?
        .join(CONFIG_DIR_NAME);
    let config_path = config_dir_path.join(CONFIG_FILE_NAME);

    let config = Config {
        connections: connections.clone(),
    };

    let toml_string = toml::to_string(&config)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    fs::write(config_path, toml_string)?;
    Ok(())
}
