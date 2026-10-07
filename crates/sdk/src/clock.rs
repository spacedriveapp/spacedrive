//! The host's clock
//!
//! wasm32-unknown-unknown has no clock, so `Utc::now()` and
//! `SystemTime::now()` trap in a guest. Extensions ask the host instead.

use chrono::{DateTime, Utc};

/// The current time, from the host.
pub fn now() -> DateTime<Utc> {
	DateTime::from_timestamp_millis(crate::ffi::now_ms()).unwrap_or_default()
}

/// Milliseconds since the Unix epoch, from the host.
pub fn now_ms() -> i64 {
	crate::ffi::now_ms()
}
