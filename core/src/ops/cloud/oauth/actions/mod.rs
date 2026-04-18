//! Wire-registered actions and queries for the OAuth subsystem.
//!
//! The `complete` action is intentionally private — it is invoked by the
//! loopback callback task, not exposed over the RPC surface. This keeps the
//! wire surface to three entries: `start`, `poll`, `cancel`.

pub mod cancel;
pub mod complete;
pub mod poll;
pub mod start;
