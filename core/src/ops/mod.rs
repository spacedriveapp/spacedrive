//! Operations module - contains all business operations and use cases
//!
//! This module organizes all business operations for Spacedrive:
//! - Addressing operations (path resolution)
//! - File operations (copy, move, delete, validate, duplicate detection)
//! - Indexing operations
//! - Media processing (thumbnails, etc.)
//! - Content operations (deduplication, statistics)
//! - Metadata operations (hierarchical tagging)

pub mod addressing;
pub mod content;
pub mod core;
pub mod devices;
pub mod entries;
pub mod files;
pub mod indexing;
pub mod libraries;
pub mod locations;
pub mod media;
pub mod metadata;
pub mod sidecar;
pub mod transport;
pub mod volumes;

/// Register all jobs with the job system
///
/// This should be called during core initialization to register all available job types
pub fn register_all_jobs() {
	// File operation jobs
	register_job::<files::copy::FileCopyJob>();
	register_job::<files::copy::MoveJob>();
	register_job::<files::delete::DeleteJob>();
	register_job::<files::duplicate_detection::DuplicateDetectionJob>();
	register_job::<files::validation::ValidationJob>();

	// Indexing jobs
	register_job::<indexing::IndexerJob>();

	// Media processing jobs
	register_job::<media::ThumbnailJob>();
}

/// Register a single job type with the job system
///
/// This function would be called automatically by a derive macro in a real implementation,
/// but for now we call it manually for each job type.
fn register_job<T>()
where
	T: crate::infra::job::traits::Job + 'static,
{
	// In a real implementation with inventory, this would automatically register the job
	// For now, this serves as documentation of which jobs should be registered

	// The actual registration would happen via:
	// inventory::submit! {
	//     crate::infra::job::registration::JobRegistration::new::<T>()
	// }

	// For now we'll just log that the job type exists
}
