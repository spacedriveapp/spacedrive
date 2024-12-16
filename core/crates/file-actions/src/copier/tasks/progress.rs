use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CopyProgress {
    // Overall progress
    Started {
        total_files: u64,
        total_bytes: u64,
    },
    // Individual file progress
    File {
        name: String,
        current_file: u64,
        total_files: u64,
        bytes: u64,
        source: PathBuf,
        target: PathBuf,
    },
    // Progress within current file (for streaming copy)
    FileProgress {
        name: String,
        bytes_copied: u64,
        total_bytes: u64,
    },
    // Completed
    Completed {
        files_copied: u64,
        bytes_copied: u64,
    }
}
