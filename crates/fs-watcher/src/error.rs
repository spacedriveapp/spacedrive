//! Error types for the filesystem watcher

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for watcher operations
pub type Result<T> = std::result::Result<T, WatcherError>;

/// Errors that can occur during filesystem watching
#[derive(Debug, Error)]
pub enum WatcherError {
	/// Failed to start the watcher
	#[error("Failed to start watcher: {0}")]
	StartFailed(String),

	/// Failed to watch a path
	#[error("Failed to watch path {path}: {reason}")]
	WatchFailed { path: PathBuf, reason: String },

	/// Failed to unwatch a path
	#[error("Failed to unwatch path {path}: {reason}")]
	UnwatchFailed { path: PathBuf, reason: String },

	/// Path does not exist
	#[error("Path does not exist: {0}")]
	PathNotFound(PathBuf),

	/// Path is not a directory
	#[error("Path is not a directory: {0}")]
	NotADirectory(PathBuf),

	/// Watcher is already running
	#[error("Watcher is already running")]
	AlreadyRunning,

	/// Watcher is not running
	#[error("Watcher is not running")]
	NotRunning,

	/// Event channel closed
	#[error("Event channel closed")]
	ChannelClosed,

	/// Internal notify error
	#[error("Notify error: {0}")]
	NotifyError(#[from] notify::Error),

	/// IO error
	#[error("IO error: {0}")]
	IoError(#[from] std::io::Error),

	/// Configuration error
	#[error("Configuration error: {0}")]
	ConfigError(String),
}
