//! Extension configuration
//!
//! The host reads `config.json` beside the extension's manifest and answers
//! `config.get` with it, or with `{}` when there is none.

use serde::de::DeserializeOwned;

/// Load the configuration once per call and leak it: a config is read a
/// handful of times per job and the guest's heap goes with the job's
/// instance, so a reference is the convenient shape and the leak is bounded.
pub fn load<C: DeserializeOwned + 'static>() -> &'static C {
	let value: serde_json::Value = crate::ffi::op_json("config.get", &serde_json::json!({}))
		.unwrap_or_else(|e| panic!("extension config could not be read from the host: {e}"));
	let config: C = serde_json::from_value(value)
		.unwrap_or_else(|e| panic!("extension config.json does not match the config type: {e}"));
	Box::leak(Box::new(config))
}
