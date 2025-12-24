//! Stale Detection Service
//!
//! Detects filesystem changes that occurred while Spacedrive was offline by leveraging
//! the indexer with modified-time pruning. The service spawns IndexerJob instances
//! with `IndexMode::Stale` which compares directory timestamps to skip unchanged branches.
//!
//! ## Service Architecture
//!
//! The service runs per-location workers that periodically check for staleness:
//! 1. Load locations with stale detection enabled
//! 2. For each location, spawn a worker task
//! 3. Workers check periodically based on configured interval
//! 4. When staleness detected, spawn IndexerJob with mtime pruning
//!
//! ## Trigger Conditions
//!
//! Stale detection triggers when:
//! - Application starts after being offline longer than threshold
//! - Watcher was interrupted (crash/force-quit)
//! - Manual trigger via UI
//! - Periodic interval check

pub mod integration;
mod service;

pub use service::{StaleDetectionService, StaleDetectionServiceConfig};
