//! # Cloud operations
//!
//! `core::ops::cloud` groups provider-agnostic cloud infrastructure that is not
//! tied to a specific volume. OAuth 2.0 sign-in flows live here so future cloud
//! features (device-code flow, change-detection schedulers, account-level
//! metadata) can share the same home without bloating `ops::volumes`.

pub mod change_detection;
pub mod oauth;
