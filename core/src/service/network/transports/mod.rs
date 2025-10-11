//! Outbound message transports
//!
//! Transports are responsible for initiating messages to remote devices.
//! This is distinct from protocol handlers which receive and respond to incoming messages.
//!
//! ## Architecture
//!
//! - `protocol/` - Inbound message handlers (implement ProtocolHandler)
//! - `transports/` - Outbound message senders (implement domain-specific transport traits)
//!
//! ## When to Use Transports vs Protocols
//!
//! **Use a Transport when:**
//! - You need to initiate messages from application logic (not in response to a network message)
//! - Example: Broadcasting sync changes when local data is modified
//!
//! **Use a Protocol when:**
//! - You're handling incoming network messages
//! - Example: Receiving and applying sync changes from peers

pub mod sync;

// Re-export for convenience
pub use sync::*;
