use crate::globals;
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct WindowConfig {
    width: u64,
    height: u64,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 440,
            height: 440,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    window_config: WindowConfig,
}

impl Config {
    fn from_file() -> Result<Self, std::io::Error> {
        let path = Self::path();
        log::info!("Loading config file: {}", path.display());

        let file = File::open(path);
        if let Err(err) = &file {
            if matches!(err.kind(), std::io::ErrorKind::NotFound) {
                log::info!("File not found. Using default value.");
                return Ok(Default::default());
            }
        }

        Ok(serde_json::de::from_reader(file?)?)
    }

    fn path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(globals::APP_ID);
        path.push("config.json");

        path
    }
}
