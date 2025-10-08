//! Sync protocol for push-based library synchronization
//!
//! This protocol enables efficient, real-time sync between leader and follower devices
//! by using push notifications instead of polling.

pub mod handler;
pub mod messages;

pub use handler::SyncProtocolHandler;
pub use messages::SyncMessage;
