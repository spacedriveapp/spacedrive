//! CLI adapter for file copy operations

use crate::ops::files::copy::input::{CopyMethod, FileCopyInput};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// CLI-specific copy method values
#[derive(Debug, Clone, ValueEnum)]
pub enum CopyMethodCli {
    /// Automatically select the best method based on source and destination
    Auto,
    /// Use atomic move (rename) for same-volume operations
    AtomicMove,
    /// Use streaming copy for cross-volume operations
    Streaming,
}

impl Default for CopyMethodCli {
    fn default() -> Self {
        CopyMethodCli::Auto
    }
}

impl From<CopyMethodCli> for CopyMethod {
    fn from(cli_method: CopyMethodCli) -> Self {
        match cli_method {
            CopyMethodCli::Auto => CopyMethod::Auto,
            CopyMethodCli::AtomicMove => CopyMethod::AtomicMove,
            CopyMethodCli::Streaming => CopyMethod::StreamingCopy,
        }
    }
}

/// CLI-specific arguments for file copy command
/// This struct handles CLI parsing and converts to the core FileCopyInput type
#[derive(Debug, Clone, Parser)]
pub struct FileCopyCliArgs {
    /// Source files or directories to copy
    pub sources: Vec<PathBuf>,

    /// Destination path
    #[arg(short = 'o', long)]
    pub destination: PathBuf,

    /// Overwrite existing files
    #[arg(long)]
    pub overwrite: bool,

    /// Verify checksums during copy
    #[arg(long)]
    pub verify: bool,

    /// Preserve file timestamps (default: true)
    #[arg(long, default_value = "true")]
    pub preserve_timestamps: bool,

    /// Move files instead of copying (delete source after copy)
    #[arg(long)]
    pub move_files: bool,

    /// Copy method to use (auto, atomic-move, streaming)
    #[arg(long, value_enum, default_value = "auto")]
    pub method: CopyMethodCli,
}

impl From<FileCopyCliArgs> for FileCopyInput {
    fn from(args: FileCopyCliArgs) -> Self {
        Self {
            sources: args.sources,
            destination: args.destination,
            overwrite: args.overwrite,
            verify_checksum: args.verify,
            preserve_timestamps: args.preserve_timestamps,
            move_files: args.move_files,
            copy_method: args.method.into(),
        }
    }
}

impl FileCopyCliArgs {
    /// Convert to core input type
    pub fn to_input(self) -> FileCopyInput {
        self.into()
    }
    
    /// Validate CLI arguments and convert to input
    pub fn validate_and_convert(self) -> Result<FileCopyInput, String> {
        let input = self.to_input();
        
        match input.validate() {
            Ok(()) => Ok(input),
            Err(errors) => Err(errors.join("; ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_to_input_conversion() {
        let cli_args = FileCopyCliArgs {
            sources: vec!["/file1.txt".into(), "/file2.txt".into()],
            destination: "/dest/".into(),
            overwrite: true,
            verify: false,
            preserve_timestamps: true,
            move_files: false,
            method: CopyMethodCli::Auto,
        };

        let input: FileCopyInput = cli_args.into();

        assert_eq!(input.sources.len(), 2);
        assert_eq!(input.destination, PathBuf::from("/dest/"));
        assert!(input.overwrite);
        assert!(!input.verify_checksum);
        assert!(input.preserve_timestamps);
        assert!(!input.move_files);
        assert_eq!(input.copy_method, CopyMethod::Auto);
    }

    #[test]
    fn test_validate_and_convert_success() {
        let cli_args = FileCopyCliArgs {
            sources: vec!["/file.txt".into()],
            destination: "/dest/".into(),
            overwrite: false,
            verify: true,
            preserve_timestamps: false,
            move_files: true,
            method: CopyMethodCli::Streaming,
        };

        let result = cli_args.validate_and_convert();
        assert!(result.is_ok());

        let input = result.unwrap();
        assert_eq!(input.sources, vec![PathBuf::from("/file.txt")]);
        assert!(input.verify_checksum);
        assert!(!input.preserve_timestamps);
        assert!(input.move_files);
        assert_eq!(input.copy_method, CopyMethod::StreamingCopy);
    }

    #[test]
    fn test_validate_and_convert_failure() {
        let cli_args = FileCopyCliArgs {
            sources: vec![], // Empty sources should fail
            destination: "/dest/".into(),
            overwrite: false,
            verify: false,
            preserve_timestamps: true,
            move_files: false,
            method: CopyMethodCli::Auto,
        };

        let result = cli_args.validate_and_convert();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("At least one source"));
    }

    #[test]
    fn test_default_values() {
        // Test that clap default values work as expected
        let cli_args = FileCopyCliArgs {
            sources: vec!["/file.txt".into()],
            destination: "/dest/".into(),
            overwrite: false,
            verify: false,
            preserve_timestamps: true, // Should default to true
            move_files: false,
            method: CopyMethodCli::Auto, // Should default to Auto
        };

        let input = cli_args.to_input();
        assert!(input.preserve_timestamps); // Default should be true
        assert_eq!(input.copy_method, CopyMethod::Auto); // Default should be Auto
    }

    #[test]
    fn test_copy_method_conversion() {
        // Test all copy method variants
        assert_eq!(CopyMethod::from(CopyMethodCli::Auto), CopyMethod::Auto);
        assert_eq!(CopyMethod::from(CopyMethodCli::AtomicMove), CopyMethod::AtomicMove);
        assert_eq!(CopyMethod::from(CopyMethodCli::Streaming), CopyMethod::StreamingCopy);
    }
}