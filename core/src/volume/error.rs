//! Error types for volume management operations
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::types::VolumeFingerprint;

/// Errors that can occur during volume operations
#[derive(Error, Debug)]
pub enum VolumeError {
	// Add context to all errors
	#[error("{context}: {source}")]
	WithContext {
		context: String,
		source: Box<VolumeError>,
	},

	// Add operation-specific errors
	#[error("Operation {0} failed: {1}")]
	OperationFailed(uuid::Uuid, String),

	/// Failed to perform I/O operation
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),

	/// Operation timed out
	#[error("Operation timed out after {0} seconds")]
	Timeout(u64),

	/// No mount point found for volume
	#[error("No mount point found for volume")]
	NoMountPoint,

	/// Volume is already mounted
	#[error("Volume with fingerprint {} is not found", 0)]
	NotFound(VolumeFingerprint),

	/// Volume isn't in database yet
	#[error("Volume not yet tracked in database")]
	NotInDatabase,

	/// Invalid volume ID
	#[error("Invalid volume fingerprint: {0}")]
	InvalidFingerprint(VolumeFingerprint),

	/// Directory operation failed
	#[error("Directory operation failed: {0}")]
	DirectoryError(String),

	/// Database operation failed
	#[error("Database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),

	/// Device error
	#[error("Device error: {0}")]
	DeviceError(String),

	/// No device found
	#[error("No device found in database")]
	NoDeviceFound,

	/// Volume already exists
	#[error("Volume already exists at path: {}", .0.display())]
	VolumeExists(PathBuf),

	/// Volume is not mounted
	#[error("Volume is not mounted: {}", .0.display())]
	NotMounted(PathBuf),

	/// Volume is read-only
	#[error("Volume is read-only: {}", .0.display())]
	ReadOnly(PathBuf),

	/// Device not found
	#[error("Device not found: {:?}", .0)]
	DeviceNotFound(Vec<u8>),

	/// Volume does not have enough space
	#[error("Insufficient space on volume: {} (needed: {needed} bytes, available: {available} bytes)", .path.display())]
	InsufficientSpace {
		path: PathBuf,
		needed: u64,
		available: u64,
	},

	/// Speed test error
	#[error("Speed test failed: {kind}: {message}")]
	SpeedTest {
		kind: SpeedTestErrorKind,
		message: String,
	},

	/// Watcher error
	#[error("Volume watcher error: {0}")]
	Watcher(#[from] WatcherError),

	/// Platform-specific error
	#[error("Platform error: {0}")]
	Platform(String),

	/// Permission denied
	#[error("Permission denied for path: {}", .0.display())]
	PermissionDenied(PathBuf),

	/// Volume is busy
	#[error("Volume is busy: {}", .0.display())]
	VolumeBusy(PathBuf),

	/// Operation cancelled
	#[error("Operation was cancelled")]
	Cancelled,

	/// Invalid configuration
	#[error("Invalid configuration: {0}")]
	InvalidConfiguration(String),

	/// Resource exhausted
	#[error("Resource exhausted: {0}")]
	ResourceExhausted(String),
}

/// Specific kinds of speed test errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpeedTestErrorKind {
	/// Failed to create test file
	CreateFile,
	/// Failed to write test data
	Write,
	/// Failed to read test data
	Read,
	/// Failed to cleanup test files
	Cleanup,
	/// Test exceeded timeout
	Timeout,
}

/// Errors specific to volume watching
#[derive(Error, Debug)]
pub enum WatcherError {
	/// Failed to initialize watcher
	#[error("Failed to initialize watcher: {0}")]
	InitializationError(String),

	/// Failed to add watch
	#[error("Failed to add watch for path: {}", .0.display())]
	AddWatchError(PathBuf),

	/// Failed to remove watch
	#[error("Failed to remove watch for path: {}", .0.display())]
	RemoveWatchError(PathBuf),

	/// Event stream error
	#[error("Event stream error: {0}")]
	EventStreamError(String),
}

// Implement conversion from tokio::time::error::Elapsed
impl From<tokio::time::error::Elapsed> for VolumeError {
	fn from(error: tokio::time::error::Elapsed) -> Self {
		VolumeError::Timeout(30) // Default timeout value
	}
}

impl VolumeError {
	/// Creates a new speed test error
	pub fn speed_test(kind: SpeedTestErrorKind, message: impl Into<String>) -> Self {
		VolumeError::SpeedTest {
			kind,
			message: message.into(),
		}
	}

	/// Checks if the error is a timeout
	pub fn is_timeout(&self) -> bool {
		matches!(self, VolumeError::Timeout(_))
	}

	/// Checks if the error is permission related
	pub fn is_permission_denied(&self) -> bool {
		matches!(self, VolumeError::PermissionDenied(_))
	}

	/// Checks if the error is space related
	pub fn is_space_error(&self) -> bool {
		matches!(self, VolumeError::InsufficientSpace { .. })
	}

	/// Checks if the operation can be retried
	pub fn is_retriable(&self) -> bool {
		matches!(
			self,
			VolumeError::Timeout(_)
				| VolumeError::VolumeBusy(_)
				| VolumeError::ResourceExhausted(_)
		)
	}

	/// Gets the path associated with the error, if any
	pub fn path(&self) -> Option<&Path> {
		match self {
			VolumeError::VolumeExists(path)
			| VolumeError::NotMounted(path)
			| VolumeError::ReadOnly(path)
			| VolumeError::InsufficientSpace { path, .. }
			| VolumeError::PermissionDenied(path)
			| VolumeError::VolumeBusy(path) => Some(path),
			_ => None,
		}
	}
}

// Implement conversion from VolumeError to rspc::Error for API responses
impl From<VolumeError> for rspc::Error {
	fn from(err: VolumeError) -> Self {
		// Map error types to appropriate HTTP status codes
		let code = match &err {
			VolumeError::NotInDatabase
			| VolumeError::NoMountPoint
			| VolumeError::InvalidFingerprint(_) => rspc::ErrorCode::NotFound,

			VolumeError::PermissionDenied(_) => rspc::ErrorCode::Forbidden,

			VolumeError::Timeout(_) | VolumeError::VolumeBusy(_) => rspc::ErrorCode::Timeout,

			VolumeError::InsufficientSpace { .. } => rspc::ErrorCode::PayloadTooLarge,

			VolumeError::InvalidConfiguration(_) => rspc::ErrorCode::BadRequest,

			_ => rspc::ErrorCode::InternalServerError,
		};

		rspc::Error::with_cause(code, err.to_string(), err)
	}
}

// Helper trait for Result extension methods
pub trait VolumeResultExt<T> {
	/// Adds context to an error
	fn with_context(self, context: impl FnOnce() -> String) -> Result<T, VolumeError>;

	/// Adds path context to an error
	fn with_path(self, path: impl AsRef<Path>) -> Result<T, VolumeError>;
}

impl<T> VolumeResultExt<T> for Result<T, VolumeError> {
	fn with_context(self, context: impl FnOnce() -> String) -> Result<T, VolumeError> {
		self.map_err(|e| VolumeError::DirectoryError(format!("{}: {}", context(), e)))
	}

	fn with_path(self, path: impl AsRef<Path>) -> Result<T, VolumeError> {
		self.map_err(|e| match e {
			VolumeError::Io(io_err) => match io_err.kind() {
				std::io::ErrorKind::PermissionDenied => {
					VolumeError::PermissionDenied(path.as_ref().to_path_buf())
				}
				_ => VolumeError::DirectoryError(format!(
					"Operation failed on path '{}': {}",
					path.as_ref().display(),
					io_err
				)),
			},
			other => other,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_error_conversion() {
		let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
		let volume_error = VolumeError::from(io_error);
		assert!(matches!(volume_error, VolumeError::Io(_)));
	}

	#[test]
	fn test_error_context() {
		let result: Result<(), VolumeError> = Err(VolumeError::NoMountPoint);
		let with_context = result.with_context(|| "Failed to mount volume".to_string());
		assert!(with_context.is_err());
	}

	#[test]
	fn test_error_helpers() {
		let error = VolumeError::InsufficientSpace {
			path: PathBuf::from("/test"),
			needed: 1000,
			available: 500,
		};
		assert!(error.is_space_error());
		assert!(!error.is_timeout());
		assert!(error.path().is_some());
	}
}

impl fmt::Display for SpeedTestErrorKind {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let kind_str = match self {
			SpeedTestErrorKind::CreateFile => "Failed to create test file",
			SpeedTestErrorKind::Write => "Failed to write test data",
			SpeedTestErrorKind::Read => "Failed to read test data",
			SpeedTestErrorKind::Cleanup => "Failed to cleanup test files",
			SpeedTestErrorKind::Timeout => "Test exceeded timeout",
		};
		write!(f, "{}", kind_str)
	}
}
