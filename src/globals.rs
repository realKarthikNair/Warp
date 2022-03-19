use once_cell::sync::Lazy;
use wormhole::transfer::AppVersion;
use wormhole::{AppConfig, AppID};

pub const WORMHOLE_RENDEZVOUS_RELAY: &str = "ws://relay.magic-wormhole.io:4000/v1";
pub const WORMHOLE_TRANSIT_RELAY: &str = "tcp://transit.magic-wormhole.io:4001";
pub static WORMHOLE_APPCFG: Lazy<AppConfig<AppVersion>> = Lazy::new(|| AppConfig {
    id: AppID::new("lothar.com/wormhole/text-or-file-xfer"),
    rendezvous_url: WORMHOLE_RENDEZVOUS_RELAY.into(),
    app_version: AppVersion {},
});
