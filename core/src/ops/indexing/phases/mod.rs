//! The walk, and what it produces.
//!
//! Discovery reads the filesystem and collects raw metadata in batches, which
//! the job then applies to the arena through `ArenaWriter`. Discovery is
//! checkpointed, so an interrupted walk resumes without re-reading directories
//! it has already been through.

pub mod discovery;

pub use discovery::run_discovery_phase;
