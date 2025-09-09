//! Core input types for file copy operations

use super::job::CopyOptions;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Copy method preference for file operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CopyMethod {
    /// Automatically select the best method based on source and destination
    Auto,
    /// Use atomic move (rename) for same-volume operations
    AtomicMove,
    /// Use streaming copy for cross-volume operations
    StreamingCopy,
}

impl Default for CopyMethod {
    fn default() -> Self {
        CopyMethod::Auto
    }
}

/// Core input structure for file copy operations
/// This is the canonical interface that all external APIs (CLI, GraphQL, REST) convert to
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCopyInput {
    /// The library ID where this operation takes place
    pub library_id: Option<uuid::Uuid>,

    /// Source files or directories to copy
    pub sources: Vec<PathBuf>,

    /// Destination path
    pub destination: PathBuf,

    /// Whether to overwrite existing files
    pub overwrite: bool,

    /// Whether to verify checksums during copy
    pub verify_checksum: bool,

    /// Whether to preserve file timestamps
    pub preserve_timestamps: bool,

    /// Whether to delete source files after copying (move operation)
    pub move_files: bool,

    /// Preferred copy method to use
    pub copy_method: CopyMethod,
}

impl FileCopyInput {
    /// Create a new FileCopyInput with default options
    pub fn new<D: Into<PathBuf>>(library_id: uuid::Uuid, sources: Vec<PathBuf>, destination: D) -> Self {
        Self {
            library_id: Some(library_id),
            sources,
            destination: destination.into(),
            overwrite: false,
            verify_checksum: false,
            preserve_timestamps: true,
            move_files: false,
            copy_method: CopyMethod::Auto,
        }
    }

    /// Create a single file copy input
    pub fn single_file<S: Into<PathBuf>, D: Into<PathBuf>>(source: S, destination: D) -> Self {
        // Placeholder: require caller to set library_id later
        Self::new(uuid::Uuid::nil(), vec![source.into()], destination)
    }

    /// Set overwrite option
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// Set checksum verification option
    pub fn with_verification(mut self, verify: bool) -> Self {
        self.verify_checksum = verify;
        self
    }

    /// Set timestamp preservation option
    pub fn with_timestamp_preservation(mut self, preserve: bool) -> Self {
        self.preserve_timestamps = preserve;
        self
    }

    /// Set move files option
    pub fn with_move(mut self, move_files: bool) -> Self {
        self.move_files = move_files;
        self
    }

    /// Set copy method preference
    pub fn with_copy_method(mut self, copy_method: CopyMethod) -> Self {
        self.copy_method = copy_method;
        self
    }

    /// Convert to CopyOptions for the job system
    pub fn to_copy_options(&self) -> CopyOptions {
        CopyOptions {
            overwrite: self.overwrite,
            verify_checksum: self.verify_checksum,
            preserve_timestamps: self.preserve_timestamps,
            delete_after_copy: self.move_files,
            move_mode: None, // Will be determined by job system
            copy_method: self.copy_method.clone(),
        }
    }

    /// Validate the input
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.sources.is_empty() {
            errors.push("At least one source file must be specified".to_string());
        }

        // Validate each source path (basic validation - existence check done in builder)
        for source in &self.sources {
            if source.as_os_str().is_empty() {
                errors.push("Source path cannot be empty".to_string());
            }
        }

        // Validate destination
        if self.destination.as_os_str().is_empty() {
            errors.push("Destination path cannot be empty".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get a summary string for logging/display
    pub fn summary(&self) -> String {
        let operation = if self.move_files { "Move" } else { "Copy" };
        let source_count = self.sources.len();
        let source_desc = if source_count == 1 {
            "1 source".to_string()
        } else {
            format!("{} sources", source_count)
        };

        format!(
            "{} {} to {}",
            operation,
            source_desc,
            self.destination.display()
        )
    }
}

impl Default for FileCopyInput {
    fn default() -> Self {
        Self {
            library_id: None,
            sources: Vec::new(),
            destination: PathBuf::new(),
            overwrite: false,
            verify_checksum: false,
            preserve_timestamps: true,
            move_files: false,
            copy_method: CopyMethod::Auto,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_input() {
        let input = FileCopyInput::new(
            uuid::Uuid::nil(),
            vec!["/file1.txt".into(), "/file2.txt".into()],
            "/dest/"
        );

        assert_eq!(input.sources.len(), 2);
        assert_eq!(input.destination, PathBuf::from("/dest/"));
        assert!(!input.overwrite);
        assert!(input.preserve_timestamps);
        assert!(!input.move_files);
    }

    #[test]
    fn test_single_file() {
        let input = FileCopyInput::single_file("/source.txt", "/dest.txt");

        assert_eq!(input.sources, vec![PathBuf::from("/source.txt")]);
        assert_eq!(input.destination, PathBuf::from("/dest.txt"));
    }

    #[test]
    fn test_fluent_api() {
        let input = FileCopyInput::single_file("/source.txt", "/dest.txt")
            .with_overwrite(true)
            .with_verification(true)
            .with_timestamp_preservation(false)
            .with_move(true);

        assert!(input.overwrite);
        assert!(input.verify_checksum);
        assert!(!input.preserve_timestamps);
        assert!(input.move_files);
    }

    #[test]
    fn test_validation_empty_sources() {
        let input = FileCopyInput::default();
        let result = input.validate();

        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("At least one source")));
    }

    #[test]
    fn test_validation_empty_destination() {
        let mut input = FileCopyInput::default();
        input.sources = vec!["/file.txt".into()];

        let result = input.validate();
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Destination path cannot be empty")));
    }

    #[test]
    fn test_validation_success() {
        let input = FileCopyInput::new(uuid::Uuid::nil(), vec!["/file.txt".into()], "/dest/");
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_summary() {
        let input = FileCopyInput::new(uuid::Uuid::nil(), vec!["/file1.txt".into(), "/file2.txt".into()], "/dest/");
        assert_eq!(input.summary(), "Copy 2 sources to /dest/");

        let move_input = input.with_move(true);
        assert_eq!(move_input.summary(), "Move 2 sources to /dest/");

        let single_input = FileCopyInput::single_file("/file.txt", "/dest.txt");
        assert_eq!(single_input.summary(), "Copy 1 source to /dest.txt");
    }

    #[test]
    fn test_to_copy_options() {
        let input = FileCopyInput::single_file("/source.txt", "/dest.txt")
            .with_overwrite(true)
            .with_verification(true)
            .with_timestamp_preservation(false)
            .with_move(true);

        let options = input.to_copy_options();
        assert!(options.overwrite);
        assert!(options.verify_checksum);
        assert!(!options.preserve_timestamps);
        assert!(options.delete_after_copy);
    }
}