use crate::globals;
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct WindowConfig {
    pub width: i32,
    pub height: i32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 440,
            height: 440,
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Config {
    pub window: WindowConfig,
}

impl Config {
    pub fn from_file() -> Result<Self, std::io::Error> {
        let path = Self::path();
        log::info!("Loading config file: '{}'", path.display());

        let file = File::open(path);
        if let Err(err) = &file {
            if matches!(err.kind(), std::io::ErrorKind::NotFound) {
                log::info!("File not found. Using default value.");
                return Ok(Default::default());
            } else {
                log::error!("Unable to load config file: {:?}", err.kind());
            }
        }

        Ok(serde_json::de::from_reader(file?)?)
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::path();
        std::fs::create_dir_all(&path.parent().unwrap())?;
        log::info!("Saving config file to: '{}'", path.display());

        let file = File::create(&path)?;
        Ok(serde_json::ser::to_writer_pretty(&file, self)?)
    }

    pub fn path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(globals::APP_ID);
        path.push("config.json");

        path
    }
}
