//! Sync Metrics and Observability System
//!
//! Provides comprehensive metrics collection and monitoring for the sync system.
//! Tracks state transitions, operation counts, data volumes, performance metrics,
//! and error events to enable debugging and monitoring.

pub mod collector;
pub mod history;
pub mod persistence;
pub mod snapshot;
pub mod types;

pub use collector::SyncMetricsCollector;
pub use types::*;
