//! # Thumbnail operations
//!
//! Viewport requests queue missing or stale tiles in draw order. Explicit
//! generation jobs select a file or indexed directory and can force a rebake.
//! Both publish through the same cache writer and thumbnail resource events.

pub mod generate;
pub mod request;
pub mod thumbstrip;
