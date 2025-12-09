//! Event handlers for filesystem changes
//!
//! These handlers subscribe to `FsWatcher` events and route them to the
//! appropriate storage layer (database for persistent, memory for ephemeral).

mod ephemeral;
mod persistent;

pub use ephemeral::EphemeralEventHandler;
pub use persistent::{LocationMeta, PersistentEventHandler};
