//! Job system for Spacedrive
//! 
//! Provides a minimal-boilerplate job execution framework built on top of the task-system.

pub mod context;
pub mod database;
pub mod error;
pub mod executor;
pub mod generic_progress;
pub mod handle;
pub mod manager;
pub mod output;
pub mod progress;
pub mod registry;
pub mod traits;
pub mod types;

#[cfg(test)]
mod manager_test;

// Re-export commonly used types
pub mod prelude {
    pub use super::{
        context::JobContext,
        error::{JobError, JobResult},
        generic_progress::{GenericProgress, ToGenericProgress},
        handle::JobHandle,
        output::JobOutput,
        progress::{JobProgress, Progress},
        traits::{Job, JobHandler},
        types::{JobId, JobStatus, JobInfo},
    };
    
    // Re-export derive macros
    pub use spacedrive_jobs_derive::Job;
}

pub use manager::JobManager;
pub use registry::JobRegistry;
pub use types::{JobInfo, JobStatus};