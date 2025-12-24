//! # Stale Detection Service
//!
//! `core::service::stale_detector` manages intelligent detection of filesystem changes
//! that occurred while Spacedrive was offline or not watching a location. Uses modified-time
//! pruning to efficiently scan only changed directory branches.
//!
//! ## Core Functionality
//!
//! - Per-location workers that trigger stale detection based on configuration
//! - Integration with IndexerJob using `IndexMode::Stale` for mtime pruning
//! - History tracking of detection runs and efficiency metrics
//! - Configurable aggressiveness levels and offline thresholds
//!
//! ## Architecture
//!
//! The service maintains per-location worker tasks that check periodically based on
//! location settings. When staleness is detected, the service spawns an IndexerJob with
//! `IndexMode::Stale` wrapping the location's configured indexing mode. The indexer's
//! discovery phase handles the actual mtime pruning and selective traversal.

mod service;
mod worker;

pub use service::StaleDetectionService;
pub use worker::LocationWorker;
