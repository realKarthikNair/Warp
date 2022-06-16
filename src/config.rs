use crate::globals;
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use wormhole::transfer::AppVersion;
use wormhole::{AppConfig, AppID};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WindowConfig {
    pub width: i32,
    pub height: i32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 460,
            height: 500,
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub window: WindowConfig,
    pub welcome_window_shown: bool,

    pub rendezvous_server_url: Option<String>,
    pub transit_server_url: Option<String>,

    pub code_length: Option<usize>,
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
                return Ok(PersistentConfig::default());
            }

            log::error!("Unable to load config file: {:?}", err.kind());
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
            let dir = Self::dir();
            std::fs::create_dir_all(&dir)?;

            let temp = tempfile::NamedTempFile::new_in(dir)?;
            serde_json::ser::to_writer_pretty(&temp, &self.config)?;

            let path = Self::path();
            log::info!("Saving config file to: '{}'", path.display());
            temp.persist(&path)?;

            self.persisted_config = self.config.clone();

            Ok(())
        }
    }

    pub fn dir() -> PathBuf {
        let mut path = glib::user_config_dir();
        path.push(globals::APP_NAME);
        path
    }

    pub fn path() -> PathBuf {
        let mut path = Self::dir();
        path.push("config.json");
        path
    }

    pub fn rendezvous_server_url_or_default(&self) -> String {
        if let Some(url) = &self.rendezvous_server_url {
            url.to_string()
        } else {
            globals::WORMHOLE_DEFAULT_RENDEZVOUS_SERVER.to_string()
        }
    }

    pub fn transit_server_url_or_default(&self) -> String {
        if let Some(url) = &self.transit_server_url {
            url.to_string()
        } else {
            globals::WORMHOLE_DEFAULT_TRANSIT_RELAY.to_string()
        }
    }

    pub fn code_length_or_default(&self) -> usize {
        self.code_length.unwrap_or(4)
    }

    pub fn app_cfg(&self) -> AppConfig<AppVersion> {
        let rendezvous_url = self
            .rendezvous_server_url_or_default()
            .trim_end_matches("/v1")
            .trim_end_matches('/')
            .to_owned();

        // Make sure we have /v1 appended exactly once
        AppConfig {
            id: AppID::new(globals::WORMHOLE_DEFAULT_APPID_STR),
            rendezvous_url: format!("{}/v1", rendezvous_url).into(),
            app_version: AppVersion {},
        }
    }
}
