use config::Config;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub username: String,
    pub server: String,
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

    pub fn save_to_file(&self, file_name: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string(self)?;

        let mut file = File::create(file_name)?;

        file.write_all(json.as_bytes())?;

        Ok(())
    }
}
