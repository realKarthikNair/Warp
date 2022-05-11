use gvdb_macros::include_gresource_from_dir;
use once_cell::sync::Lazy;
use regex::Regex;
use wormhole::transfer::AppVersion;
use wormhole::{AppConfig, AppID};

pub const WORMHOLE_RENDEZVOUS_RELAY: &str = "ws://relay.magic-wormhole.io:4000/v1";
pub const WORMHOLE_TRANSIT_RELAY: &str = "tcp://transit.magic-wormhole.io:4001";
pub static WORMHOLE_APPCFG: Lazy<AppConfig<AppVersion>> = Lazy::new(|| AppConfig {
    id: AppID::new("lothar.com/wormhole/text-or-file-xfer"),
    rendezvous_url: WORMHOLE_RENDEZVOUS_RELAY.into(),
    app_version: AppVersion {},
});

pub static TRANSMIT_CODE_FIND_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d+-[a-z]+(?:-[a-z]+)+)").unwrap());
pub static TRANSMIT_CODE_MATCH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\d+-[a-z]+(?:-[a-z]+)+$").unwrap());

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
pub const GRESOURCE_DATA: &[u8] = include_gresource_from_dir!("/app/drey/Warp", "data/resources");
