use gvdb_macros::include_gresource_from_dir;
use once_cell::sync::Lazy;
use regex::Regex;

pub static WORMHOLE_RENDEZVOUS_RELAY_DEFAULT: &str = "ws://relay.magic-wormhole.io:4000/v1";
pub static WORMHOLE_TRANSIT_RELAY_DEFAULT: &str = "tcp://transit.magic-wormhole.io:4001";

pub static TRANSMIT_CODE_FIND_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+-\S+)").unwrap());
pub static TRANSMIT_CODE_MATCH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(^\d+-\S+$)").unwrap());

#[cfg(debug_assertions)]
pub const DEBUG_BUILD: bool = true;
#[cfg(not(debug_assertions))]
pub const DEBUG_BUILD: bool = false;

pub const APP_ID: &str = if DEBUG_BUILD {
    "app.drey.Warp.Devel"
} else {
    "app.drey.Warp"
};

pub const TRANSMIT_URI_PREFIX: &str = "warp://recv/";

pub const APP_NAME: &str = "warp";
pub const GETTEXT_PACKAGE: &str = APP_NAME;
pub const DEFAULT_LOCALEDIR: &str = "/usr/share/locale";
pub const PKGDATADIR: &str = "/app/share/warp";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub static GRESOURCE_DATA: &[u8] = include_gresource_from_dir!("/app/drey/Warp", "data/resources");
