use once_cell::sync::Lazy;
use wormhole::{AppConfig, AppID};

pub const WORMHOLE_RENDEZVOUS_RELAY: &str = "ws://relay.magic-wormhole.io:4000/v1";
pub const WORMHOLE_TRANSIT_RELAY: &str = "tcp://transit.magic-wormhole.io:4001";
pub const WORMHOLE_APPCFG: Lazy<AppConfig<String>> = Lazy::new(|| AppConfig {
    id: AppID::new("lothar.com/wormhole/text-or-file-xfer"),
    rendezvous_url: WORMHOLE_RENDEZVOUS_RELAY.into(),
    app_version: "net.felinira.warp".into(),
});
