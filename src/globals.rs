use gvdb_macros::include_gresource_from_dir;
use once_cell::sync::Lazy;
use regex::Regex;
use std::sync::Mutex;

pub const WORMHOLE_DEFAULT_RENDEZVOUS_SERVER: Lazy<url::Url> =
    Lazy::new(|| url::Url::parse("ws://relay.magic-wormhole.io:4000").unwrap());
pub const WORMHOLE_DEFAULT_TRANSIT_RELAY: Lazy<url::Url> =
    Lazy::new(|| url::Url::parse("tcp://transit.magic-wormhole.io:4001").unwrap());
pub const WORMHOLE_DEFAULT_APPID_STR: &str = "lothar.com/wormhole/text-or-file-xfer";

pub static TRANSMIT_URI_FIND_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"wormhole-transfer:\d+-\S+").unwrap());
pub static TRANSMIT_CODE_FIND_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+-\S+").unwrap());
pub static TRANSMIT_CODE_MATCH_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d+-\S+$").unwrap());

#[cfg(debug_assertions)]
pub const DEBUG_BUILD: bool = true;
#[cfg(not(debug_assertions))]
pub const DEBUG_BUILD: bool = false;

pub const APP_ID: &str = if DEBUG_BUILD {
    "app.drey.Warp.Devel"
} else {
    "app.drey.Warp"
};

pub static PANIC_BACKTRACES: Lazy<Mutex<Vec<String>>> = Lazy::new(Default::default);

pub const APP_NAME: &str = "warp";
pub const GETTEXT_PACKAGE: &str = APP_NAME;
pub const DEFAULT_LOCALEDIR: &str = "/usr/share/locale";
pub const PKGDATADIR: &str = "/app/share/warp";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub static GRESOURCE_DATA: &[u8] = include_gresource_from_dir!("/app/drey/Warp", "data/resources");
