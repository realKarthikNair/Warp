use crate::service::twisted::TwistedReactor;
use once_cell::sync::Lazy;

pub const WORMHOLE_RENDEZVOUS_RELAY: &str = "ws://relay.magic-wormhole.io:4000/v1";
pub const WORMHOLE_TRANSIT_RELAY: &str = "tcp:transit.magic-wormhole.io:4001";
pub static TWISTED_REACTOR: Lazy<TwistedReactor> = Lazy::new(|| TwistedReactor::new().unwrap());
