#![warn(clippy::all, clippy::unwrap_used, clippy::panic)]
#![allow(clippy::unnecessary_cast)] // Yeah they aren't necessary on this arch, but they are on others

mod identity_or_remote_identity;
pub(super) mod libraries;
pub mod operations;
mod p2p_events;
mod p2p_manager;
mod p2p_manager_actor;
mod peer_metadata;
mod protocol;
pub mod sync;

pub use identity_or_remote_identity::*;
pub use p2p_events::*;
pub use p2p_manager::*;
pub use p2p_manager_actor::*;
pub use peer_metadata::*;
pub use protocol::*;

pub(super) const SPACEDRIVE_APP_ID: &str = "sd";
