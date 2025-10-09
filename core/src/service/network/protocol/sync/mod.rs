//! Sync protocol (Leaderless)
//!
//! Peer-to-peer sync protocol implementation

pub mod handler;
pub mod messages;

pub use handler::SyncProtocolHandler;
pub use messages::{StateRecord, SyncMessage};
