//! Stale Detection Service
//!
//! Detects stale locations (locations with changes that occurred while Spacedrive was offline)
//! using modified-time pruning to efficiently scan only changed directories.

pub mod service;

pub use service::StaleDetectionService;
