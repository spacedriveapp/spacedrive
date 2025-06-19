//! File operations - the core of what Spacedrive does
//! 
//! This module provides a unified interface for file operations,
//! handling both indexed and non-indexed files transparently.

use crate::shared::errors::FileOpError;
use std::path::PathBuf;
use tokio::fs;

pub mod copy;
// pub mod delete;
// pub mod move_files;  // Not "cut" - use clear domain language
// pub mod rename;

/// Result type for file operations
pub type Result<T> = std::result::Result<T, FileOpError>;

/// Options for file operations
#[derive(Debug, Default, Clone)]
pub struct FileOpOptions {
    /// Whether to overwrite existing files
    pub overwrite: bool,
    
    /// Whether to preserve timestamps
    pub preserve_timestamps: bool,
    
    /// Whether to update the index after operation
    pub update_index: bool,
}

/// Progress information for file operations
#[derive(Debug, Clone)]
pub struct FileOpProgress {
    /// Current file being processed
    pub current_file: PathBuf,
    
    /// Bytes processed so far
    pub bytes_processed: u64,
    
    /// Total bytes to process
    pub total_bytes: u64,
    
    /// Files processed so far
    pub files_processed: usize,
    
    /// Total files to process
    pub total_files: usize,
}

/// Result of a file operation
#[derive(Debug)]
pub struct FileOpResult {
    /// Source path
    pub source: PathBuf,
    
    /// Destination path (if applicable)
    pub destination: Option<PathBuf>,
    
    /// Whether the operation succeeded
    pub success: bool,
    
    /// Error if the operation failed
    pub error: Option<FileOpError>,
}

/// Common utilities for file operations
pub(crate) mod utils {
    use super::*;
    
    /// Check if a path is indexed
    pub async fn is_indexed(path: &PathBuf) -> Result<bool> {
        // Check if this path is within any indexed location
        // This is where we'd query the database
        Ok(false) // Placeholder
    }
    
    /// Find an available filename if the target exists
    pub async fn find_available_filename(path: &PathBuf) -> Result<PathBuf> {
        let mut candidate = path.clone();
        let mut counter = 1;
        
        while fs::try_exists(&candidate).await? {
            let stem = path.file_stem().unwrap_or_default();
            let extension = path.extension();
            
            let new_name = if let Some(ext) = extension {
                format!("{} ({})", stem.to_string_lossy(), counter)
            } else {
                format!("{} ({})", stem.to_string_lossy(), counter)
            };
            
            candidate = path.with_file_name(new_name);
            if let Some(ext) = extension {
                candidate.set_extension(ext);
            }
            
            counter += 1;
        }
        
        Ok(candidate)
    }
}