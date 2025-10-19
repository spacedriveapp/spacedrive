//! Shared utilities for the networking system

pub mod connection;
pub mod identity;
pub mod logging;

pub use connection::get_or_create_connection;
pub use identity::NetworkIdentity;
pub use logging::{ConsoleLogger, NetworkLogger, SilentLogger};
