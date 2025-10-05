//! Indexer phases implementation
//!
//! The indexer operates in distinct phases for clarity and resumability:
//! 1. Discovery - Walk directories and collect entries
//! 2. Processing - Create/update database records
//! 3. Aggregation - Calculate directory sizes
//! 4. Content - Generate content identities

pub mod aggregation;
pub mod content;
pub mod discovery;
pub mod processing;

pub use aggregation::run_aggregation_phase;
pub use content::run_content_phase;
pub use discovery::run_discovery_phase;
pub use processing::run_processing_phase;
