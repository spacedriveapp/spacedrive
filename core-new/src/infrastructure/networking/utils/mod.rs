//! Shared utilities for the networking system

pub mod identity;
pub mod logging;

pub use identity::NetworkIdentity;
pub use logging::{NetworkLogger, SilentLogger, ConsoleLogger};