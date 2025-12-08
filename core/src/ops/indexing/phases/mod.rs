//! # Indexer Execution Phases
//!
//! The indexer runs in four sequential phases to enable resumability and incremental
//! progress tracking. Each phase is independently checkpointed so interrupted jobs can
//! resume mid-phase without reprocessing completed work. This prevents re-walking large
//! directories or re-hashing files after crashes or cancellations.
//!
//! Discovery walks the filesystem and collects raw metadata. Processing converts those
//! entries into database records with stable UUIDs. Aggregation bubbles up directory
//! sizes from leaves to root (required for accurate folder size reporting). Content
//! identification hashes file contents for deduplication and generates deterministic
//! sync UUIDs.

pub mod aggregation;
pub mod content;
pub mod discovery;
pub mod processing;

pub use aggregation::run_aggregation_phase;
pub use content::run_content_phase;
pub use discovery::run_discovery_phase;
pub use processing::run_processing_phase;
