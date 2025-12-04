//! Sync event logging infrastructure
//!
//! Provides persistent logging of high-level sync events for debugging and observability.
//! Events are stored in sync.db and survive app restarts.

pub mod aggregator;
pub mod logger;
pub mod query;
pub mod types;

pub use aggregator::{BatchAggregator, BatchAggregatorConfig};
pub use logger::SyncEventLogger;
pub use query::SyncEventQuery;
pub use types::{EventCategory, EventSeverity, SyncEventLog, SyncEventType};
