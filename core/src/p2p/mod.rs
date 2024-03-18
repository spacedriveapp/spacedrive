#![warn(clippy::all, clippy::unwrap_used, clippy::panic)]
#![allow(clippy::unnecessary_cast)] // Yeah they aren't necessary on this arch, but they are on others

mod events;
pub(super) mod libraries;
mod manager;
mod metadata;
pub mod operations;
mod protocol;
pub mod sync;

pub use events::*;
pub use manager::*;
pub use metadata::*;
pub use protocol::*;

pub(super) const SPACEDRIVE_APP_ID: &str = "sd";
