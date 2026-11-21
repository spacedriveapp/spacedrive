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

/// The stamp an assertion carries: an HLC from the host's assertion clock
/// and the device that wrote it. Sync orders assertions by `hlc` with
/// `device_uuid` as the tiebreak, so anything a person decides (a name, a
/// correction) should be stamped here rather than with [`now`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stamp {
	pub hlc: String,
	pub device_uuid: uuid::Uuid,
}

/// A fresh stamp for one assertion.
pub fn stamp() -> crate::types::Result<Stamp> {
	crate::ffi::op_json("assertions.stamp", &serde_json::json!({}))
}
