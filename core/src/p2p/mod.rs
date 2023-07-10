#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Remove once this is fully stablised

mod p2p_manager;
mod peer_metadata;
mod protocol;

pub use p2p_manager::*;
pub use peer_metadata::*;
pub use protocol::*;

pub(super) const SPACEDRIVE_APP_ID: &str = "spacedrive";
