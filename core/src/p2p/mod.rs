#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Remove once this is fully stablised
#![allow(dead_code)] // TODO: Remove once protocol is finished
#![allow(clippy::unnecessary_cast)] // Yeah they aren't necessary on this arch, but they are on others

mod identity_or_remote_identity;
mod p2p_manager;
mod pairing;
mod peer_metadata;
mod protocol;
pub mod sync;

pub use identity_or_remote_identity::*;
pub use p2p_manager::*;
pub use pairing::*;
pub use peer_metadata::*;
pub use protocol::*;

pub(super) const SPACEDRIVE_APP_ID: &str = "spacedrive";
