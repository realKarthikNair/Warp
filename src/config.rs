use crate::globals;
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub window: WindowConfig,
}

#[derive(Clone, Default, Debug)]
pub struct PersistentConfig {
    pub config: Config,
    pub persisted_config: Config,
}

impl Deref for PersistentConfig {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl DerefMut for PersistentConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

impl PersistentConfig {
    pub fn from_file() -> Result<Self, std::io::Error> {
        let path = Self::path();
        log::info!("Loading config file: '{}'", path.display());

        let file = File::open(path);
        if let Err(err) = &file {
            if matches!(err.kind(), std::io::ErrorKind::NotFound) {
                log::info!("Config file not found. Using default values");
                return Ok(Default::default());
            } else {
                log::error!("Unable to load config file: {:?}", err.kind());
            }
        }

        let cfg: Config = serde_json::de::from_reader(file?)?;

        Ok(Self {
            config: cfg.clone(),
            persisted_config: cfg,
        })
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if self.config == self.persisted_config {
            log::info!("Not saving config, no values have changed");
            Ok(())
        } else {
            let path = Self::path();
            std::fs::create_dir_all(&path.parent().unwrap())?;
            log::info!("Saving config file to: '{}'", path.display());

            let file = File::create(&path)?;
            serde_json::ser::to_writer_pretty(&file, &self.config)?;
            self.persisted_config = self.config.clone();

            Ok(())
        }
    }

    pub fn path() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(globals::APP_NAME);
        path.push("config.json");

        path
    }
}
