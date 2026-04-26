use crate::{Cache, Config};
use std::path::PathBuf;

#[derive(Default)]
pub struct Persistent {
    pub config: Config,
    config_path: PathBuf,
    pub cache: Cache,
    cache_path: PathBuf,
}

impl Persistent {
    pub fn load_or_create() -> Self {
        let home_dir = std::env::home_dir().expect("Home directory not found");
        let config_dir = home_dir.join(".config/bakatui");

        let cache_file = config_dir.join("cache");
        let config_file = config_dir.join("config");

        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        }

        if !cache_file.exists() {
            std::fs::write(&cache_file, "").expect("Failed to create cache file");
        }

        if !config_file.exists() {
            std::fs::write(&config_file, "").expect("Failed to create config file");
        }

        Self {
            cache: Cache::read_from_file(&cache_file),
            config: Config::read_from_file(&config_file),
            config_path: config_file,
            cache_path: cache_file,
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        std::fs::write(
            &self.config_path,
            serde_json::to_string(&self.config).unwrap(),
        )?;
        std::fs::write(
            &self.cache_path,
            serde_json::to_string(&self.cache).unwrap(),
        )?;
        Ok(())
    }
}

impl Config {
    fn read_from_file(path: &PathBuf) -> Self {
        let contents = std::fs::read_to_string(path).unwrap_or_default();
        let config: Self = serde_json::from_str(&contents).unwrap_or_default();
        config
    }
}

impl Cache {
    fn read_from_file(path: &PathBuf) -> Self {
        let contents = std::fs::read_to_string(path).unwrap_or_default();
        let config: Self = serde_json::from_str(&contents).unwrap_or_default();
        config
    }
}
