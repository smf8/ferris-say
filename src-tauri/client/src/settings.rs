use config::Config;
use directories::ProjectDirs;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub username: String,
    pub server: String,
}

lazy_static! {
    static ref DEFAULT_CONFIG_PATH: Option<Box<Path>> = {
        let project_dir = ProjectDirs::from("com", "smf8", "x-ferris-say");
        if project_dir.is_none() {
            tracing::error!("failed to locate default config file for system. fk it I'm out.");

            return None;
        }

        let default_config_file = PathBuf::from(project_dir.unwrap().config_dir());

        Some(default_config_file.into_boxed_path())
    };
}

impl Settings {
    pub fn new(username: String, server: String) -> Self {
        Settings { username, server }
    }

    pub fn from_file(file_name: &str) -> anyhow::Result<Self> {
        let config = Config::builder()
            .add_source(config::File::with_name(file_name))
            .add_source(config::Environment::with_prefix("X_FERRIS_SAY"))
            .build()?;

        let settings = config.try_deserialize::<Settings>()?;

        Ok(settings)
    }

    pub fn from_system_path() -> anyhow::Result<Self> {
        let mut config_file = PathBuf::from(DEFAULT_CONFIG_PATH.as_ref().unwrap().as_ref());
        config_file.push("x-ferris-say.json");

        Self::from_file(config_file.to_str().unwrap())
    }

    pub fn save_to_system_path(&self) -> anyhow::Result<String> {
        fs::create_dir_all(DEFAULT_CONFIG_PATH.as_ref().unwrap())?;
        let mut config_file = PathBuf::from(DEFAULT_CONFIG_PATH.as_ref().unwrap().as_ref());
        config_file.push("x-ferris-say.json");

        self.save_to_file(config_file.to_str().unwrap())?;

        Ok(config_file.to_str().unwrap().to_owned())
    }

    pub fn save_to_file(&self, config_dir: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string(self)?;

        let mut file = File::create(config_dir)?;

        file.write_all(json.as_bytes())?;

        Ok(())
    }
}
