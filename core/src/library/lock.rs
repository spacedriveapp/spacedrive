//! Library lock implementation to prevent concurrent access

use super::error::{LibraryError, Result};
use crate::device::get_current_device_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Information stored in the lock file
#[derive(Debug, Serialize, Deserialize)]
pub struct LockInfo {
	/// ID of the device holding the lock
	pub device_id: Uuid,

	/// Process ID
	pub process_id: u32,

	/// When the lock was acquired
	pub acquired_at: DateTime<Utc>,

	/// Optional description (e.g., "indexing", "backup")
	pub description: Option<String>,
}

/// A lock that prevents concurrent access to a library
pub struct LibraryLock {
	/// Path to the lock file
	path: PathBuf,

	/// The open file handle (keeps the lock active)
	_file: File,
}

impl LibraryLock {
	/// Attempt to acquire a lock on the library
	pub fn acquire(library_path: &Path) -> Result<Self> {
		let lock_path = library_path.join(".sdlibrary.lock");

		// Try to create the lock file exclusively
		match OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&lock_path)
		{
			Ok(mut file) => {
				// Write lock information
				let lock_info = LockInfo {
					device_id: get_current_device_id(),
					process_id: std::process::id(),
					acquired_at: Utc::now(),
					description: None,
				};

				let json = serde_json::to_string_pretty(&lock_info)?;
				file.write_all(json.as_bytes())?;
				file.sync_all()?;

				Ok(Self {
					path: lock_path,
					_file: file,
				})
			}
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				// Lock file exists, check if it's stale
				if Self::is_lock_stale(&lock_path)? {
					// Remove stale lock and try again
					std::fs::remove_file(&lock_path)?;

					// Recursive call to try again
					Self::acquire(library_path)
				} else {
					Err(LibraryError::AlreadyInUse)
				}
			}
			Err(e) => Err(e.into()),
		}
	}

	/// Check if a lock file is stale (older than 1 hour or process no longer running)
	pub fn is_lock_stale(lock_path: &Path) -> Result<bool> {
		let metadata = std::fs::metadata(lock_path)?;
		let modified = metadata.modified()?;
		let age = SystemTime::now()
			.duration_since(modified)
			.unwrap_or(Duration::ZERO);

		// Consider lock stale if older than 1 hour
		if age > Duration::from_secs(3600) {
			return Ok(true);
		}

		// Also check if the process is still running
		if let Ok(contents) = std::fs::read_to_string(lock_path) {
			if let Ok(lock_info) = serde_json::from_str::<LockInfo>(&contents) {
				// Check if process is still running
				if !is_process_running(lock_info.process_id) {
					return Ok(true);
				}
			}
		}

		Ok(false)
	}

	/// Try to read lock information (for debugging)
	pub fn read_lock_info(library_path: &Path) -> Result<Option<LockInfo>> {
		let lock_path = library_path.join(".sdlibrary.lock");

		if !lock_path.exists() {
			return Ok(None);
		}

		let contents = std::fs::read_to_string(lock_path)?;
		let info: LockInfo = serde_json::from_str(&contents)?;

		Ok(Some(info))
	}

	/// Explicitly release the lock (for use during shutdown)
	/// This is called during library shutdown to ensure the lock is released
	/// even if there are lingering Arc references
	pub fn release(&mut self) {
		let _ = std::fs::remove_file(&self.path);
	}
}

/// Check if a process is still running (Unix-specific implementation)
#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
	use std::process::Command;

	match Command::new("ps").arg("-p").arg(pid.to_string()).output() {
		Ok(output) => output.status.success(),
		Err(_) => false,
	}
}

/// Check if a process is still running (Windows implementation)
#[cfg(windows)]
fn is_process_running(pid: u32) -> bool {
	use std::process::Command;

	match Command::new("tasklist")
		.arg("/fi")
		.arg(&format!("pid eq {}", pid))
		.arg("/fo")
		.arg("csv")
		.output()
	{
		Ok(output) => {
			let output_str = String::from_utf8_lossy(&output.stdout);
			output_str.lines().count() > 1 // Header + process line if exists
		}
		Err(_) => false,
	}
}

impl Drop for LibraryLock {
	fn drop(&mut self) {
		// Clean up the lock file when the lock is dropped
		let _ = std::fs::remove_file(&self.path);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	#[test]
	fn test_library_lock() {
		let temp_dir = TempDir::new().unwrap();
		let library_path = temp_dir.path().join("test.sdlibrary");
		std::fs::create_dir_all(&library_path).unwrap();

		// First lock should succeed
		let _lock1 = LibraryLock::acquire(&library_path).unwrap();

		// Second lock should fail
		match LibraryLock::acquire(&library_path) {
			Err(LibraryError::AlreadyInUse) => {}
			_ => panic!("Expected AlreadyInUse error"),
		}

		// Lock file should exist
		assert!(library_path.join(".sdlibrary.lock").exists());
	}

	#[test]
	fn test_lock_cleanup() {
		let temp_dir = TempDir::new().unwrap();
		let library_path = temp_dir.path().join("test.sdlibrary");
		std::fs::create_dir_all(&library_path).unwrap();

		{
			let _lock = LibraryLock::acquire(&library_path).unwrap();
			assert!(library_path.join(".sdlibrary.lock").exists());
		}

		// Lock file should be cleaned up after drop
		assert!(!library_path.join(".sdlibrary.lock").exists());
	}
}
