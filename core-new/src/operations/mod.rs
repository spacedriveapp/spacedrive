//! Operations module - contains all job implementations
//!
//! This module organizes all the job implementations for Spacedrive operations:
//! - File operations (copy, move, delete, validate, duplicate detection)
//! - Indexing operations
//! - Thumbnail generation
//! - Other content processing

pub mod file_ops;
pub mod indexing;
pub mod media_processing;

/// Register all jobs with the job system
/// 
/// This should be called during core initialization to register all available job types
pub fn register_all_jobs() {
    // File operation jobs
    register_job::<file_ops::copy_job::FileCopyJob>();
    register_job::<file_ops::move_job::MoveJob>();
    register_job::<file_ops::delete_job::DeleteJob>();
    register_job::<file_ops::duplicate_detection_job::DuplicateDetectionJob>();
    register_job::<file_ops::validation_job::ValidationJob>();
    
    // Indexing jobs
    register_job::<indexing::IndexerJob>();
    
    // Media processing jobs
    register_job::<media_processing::ThumbnailJob>();
}

/// Register a single job type with the job system
/// 
/// This function would be called automatically by a derive macro in a real implementation,
/// but for now we call it manually for each job type.
fn register_job<T>() 
where
    T: crate::infrastructure::jobs::traits::Job + 'static,
{
    // In a real implementation with inventory, this would automatically register the job
    // For now, this serves as documentation of which jobs should be registered
    
    // The actual registration would happen via:
    // inventory::submit! {
    //     crate::infrastructure::jobs::registration::JobRegistration::new::<T>()
    // }
    
    // For now we'll just log that the job type exists
    println!("Registering job: {}", T::NAME);
}