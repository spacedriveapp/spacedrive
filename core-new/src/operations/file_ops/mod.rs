//! File operations - comprehensive file management jobs
//! 
//! This module contains job implementations for all file operations:
//! - Copy files and directories
//! - Move/rename files and directories  
//! - Delete files (trash, permanent, secure)
//! - Duplicate detection and analysis
//! - File validation and integrity checking

pub mod copy;
pub mod delete_job;
pub mod duplicate_detection_job;
pub mod validation_job;

// Re-export the copy module types for easy access
pub use copy::{FileCopyJob, CopyOptions, MoveMode, MoveJob};

#[cfg(test)]
mod tests;

#[cfg(test)]
pub use tests::*;