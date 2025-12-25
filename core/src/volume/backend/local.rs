//! Local filesystem backend implementation

use async_trait::async_trait;
use bytes::Bytes;
use std::ops::Range;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tracing::debug;

use super::{BackendType, RawDirEntry, RawMetadata, VolumeBackend};
use crate::ops::indexing::state::EntryKind;
use crate::volume::error::VolumeError;

/// Local filesystem backend
///
/// Wraps standard filesystem operations for the volume backend trait.
/// This is a thin wrapper around tokio::fs with no additional logic.
#[derive(Debug, Clone)]
pub struct LocalBackend {
	/// Root mount point for this volume
	root: PathBuf,
}

impl LocalBackend {
	/// Create a new local backend for the given mount point
	pub fn new(root: impl Into<PathBuf>) -> Self {
		Self { root: root.into() }
	}

	/// Resolve a path relative to the volume root
	fn resolve_path(&self, path: &Path) -> PathBuf {
		if path.is_absolute() {
			path.to_path_buf()
		} else {
			self.root.join(path)
		}
	}

	/// Extract inode from metadata (platform-specific)
	#[cfg(unix)]
	fn get_inode(metadata: &std::fs::Metadata) -> Option<u64> {
		use std::os::unix::fs::MetadataExt;
		Some(metadata.ino())
	}

	#[cfg(windows)]
	fn get_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
		// Windows 'file_index' is unstable (issue #63010).
		// Returning None is safe as the field is Optional.
		None
	}

	#[cfg(not(any(unix, windows)))]
	fn get_inode(_metadata: &std::fs::Metadata) -> Option<u64> {
		None
	}
}

#[async_trait]
impl VolumeBackend for LocalBackend {
	async fn read(&self, path: &Path) -> Result<Bytes, VolumeError> {
		let full_path = self.resolve_path(path);
		debug!("LocalBackend::read: {}", full_path.display());

		let data = fs::read(&full_path).await.map_err(|e| VolumeError::Io(e))?;

		Ok(Bytes::from(data))
	}

	async fn read_range(&self, path: &Path, range: Range<u64>) -> Result<Bytes, VolumeError> {
		let full_path = self.resolve_path(path);
		debug!(
			"LocalBackend::read_range: {} ({}..{})",
			full_path.display(),
			range.start,
			range.end
		);

		let mut file = fs::File::open(&full_path)
			.await
			.map_err(|e| VolumeError::Io(e))?;

		// Seek to start position
		file.seek(std::io::SeekFrom::Start(range.start))
			.await
			.map_err(|e| VolumeError::Io(e))?;

		// Read the range
		let length = (range.end - range.start) as usize;
		let mut buffer = vec![0u8; length];
		file.read_exact(&mut buffer)
			.await
			.map_err(|e| VolumeError::Io(e))?;

		Ok(Bytes::from(buffer))
	}

	async fn write(&self, path: &Path, data: Bytes) -> Result<(), VolumeError> {
		let full_path = self.resolve_path(path);
		debug!(
			"LocalBackend::write: {} ({} bytes)",
			full_path.display(),
			data.len()
		);

		// Create parent directories if needed
		if let Some(parent) = full_path.parent() {
			fs::create_dir_all(parent)
				.await
				.map_err(|e| VolumeError::Io(e))?;
		}

		fs::write(&full_path, data)
			.await
			.map_err(|e| VolumeError::Io(e))?;

		Ok(())
	}

	async fn read_dir(&self, path: &Path) -> Result<Vec<RawDirEntry>, VolumeError> {
		let full_path = self.resolve_path(path);
		debug!("LocalBackend::read_dir: {}", full_path.display());

		let mut entries = Vec::new();
		let mut dir = fs::read_dir(&full_path)
			.await
			.map_err(|e| VolumeError::Io(e))?;

		while let Some(entry) = dir.next_entry().await.map_err(|e| VolumeError::Io(e))? {
			let metadata = match entry.metadata().await {
				Ok(m) => m,
				Err(_) => continue, // Skip entries we can't read
			};

			let kind = if metadata.is_dir() {
				EntryKind::Directory
			} else if metadata.is_symlink() {
				EntryKind::Symlink
			} else {
				EntryKind::File
			};

			entries.push(RawDirEntry {
				name: entry.file_name().to_string_lossy().to_string(),
				kind,
				size: metadata.len(),
				modified: metadata.modified().ok(),
				inode: Self::get_inode(&metadata),
			});
		}

		Ok(entries)
	}

	async fn metadata(&self, path: &Path) -> Result<RawMetadata, VolumeError> {
		let full_path = self.resolve_path(path);
		debug!("LocalBackend::metadata: {}", full_path.display());

		let metadata = fs::symlink_metadata(&full_path)
			.await
			.map_err(|e| VolumeError::Io(e))?;

		let kind = if metadata.is_dir() {
			EntryKind::Directory
		} else if metadata.is_symlink() {
			EntryKind::Symlink
		} else {
			EntryKind::File
		};

		#[cfg(unix)]
		let permissions = {
			use std::os::unix::fs::MetadataExt;
			Some(metadata.mode())
		};

		#[cfg(not(unix))]
		let permissions = None;

		Ok(RawMetadata {
			kind,
			size: metadata.len(),
			modified: metadata.modified().ok(),
			created: metadata.created().ok(),
			accessed: metadata.accessed().ok(),
			inode: Self::get_inode(&metadata),
			permissions,
		})
	}

	async fn exists(&self, path: &Path) -> Result<bool, VolumeError> {
		let full_path = self.resolve_path(path);
		Ok(full_path.exists())
	}

	async fn delete(&self, path: &Path) -> Result<(), VolumeError> {
		let full_path = self.resolve_path(path);
		debug!("LocalBackend::delete: {}", full_path.display());

		let metadata = fs::metadata(&full_path)
			.await
			.map_err(|e| VolumeError::Io(e))?;

		if metadata.is_dir() {
			fs::remove_dir_all(&full_path)
				.await
				.map_err(|e| VolumeError::Io(e))?;
		} else {
			fs::remove_file(&full_path)
				.await
				.map_err(|e| VolumeError::Io(e))?;
		}

		Ok(())
	}

	async fn create_directory(&self, path: &Path, recursive: bool) -> Result<(), VolumeError> {
		let full_path = self.resolve_path(path);
		debug!(
			"LocalBackend::create_directory: {} (recursive: {})",
			full_path.display(),
			recursive
		);

		if recursive {
			fs::create_dir_all(&full_path)
				.await
				.map_err(VolumeError::Io)?;
		} else {
			fs::create_dir(&full_path).await.map_err(VolumeError::Io)?;
		}

		Ok(())
	}

	fn is_local(&self) -> bool {
		true
	}

	fn backend_type(&self) -> BackendType {
		BackendType::Local
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	#[tokio::test]
	async fn test_local_backend_read_write() {
		let temp_dir = TempDir::new().unwrap();
		let backend = LocalBackend::new(temp_dir.path());

		let test_data = Bytes::from("Hello, world!");
		let test_path = Path::new("test.txt");

		// Write
		backend.write(test_path, test_data.clone()).await.unwrap();

		// Read
		let read_data = backend.read(test_path).await.unwrap();
		assert_eq!(test_data, read_data);
	}

	#[tokio::test]
	async fn test_local_backend_read_range() {
		let temp_dir = TempDir::new().unwrap();
		let backend = LocalBackend::new(temp_dir.path());

		let test_data = Bytes::from("0123456789");
		let test_path = Path::new("range_test.txt");

		backend.write(test_path, test_data.clone()).await.unwrap();

		// Read range [2..5] should give "234"
		let range_data = backend.read_range(test_path, 2..5).await.unwrap();
		assert_eq!(&range_data[..], b"234");
	}

	#[tokio::test]
	async fn test_local_backend_read_dir() {
		let temp_dir = TempDir::new().unwrap();
		let backend = LocalBackend::new(temp_dir.path());

		// Create test files
		backend
			.write(Path::new("file1.txt"), Bytes::from("test1"))
			.await
			.unwrap();
		backend
			.write(Path::new("file2.txt"), Bytes::from("test2"))
			.await
			.unwrap();

		// Create subdirectory
		tokio::fs::create_dir(temp_dir.path().join("subdir"))
			.await
			.unwrap();

		// Read directory
		let entries = backend.read_dir(Path::new(".")).await.unwrap();
		assert_eq!(entries.len(), 3);

		let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
		assert!(names.contains(&"file1.txt"));
		assert!(names.contains(&"file2.txt"));
		assert!(names.contains(&"subdir"));
	}

	#[tokio::test]
	async fn test_local_backend_metadata() {
		let temp_dir = TempDir::new().unwrap();
		let backend = LocalBackend::new(temp_dir.path());

		let test_data = Bytes::from("test data");
		let test_path = Path::new("metadata_test.txt");

		backend.write(test_path, test_data.clone()).await.unwrap();

		let metadata = backend.metadata(test_path).await.unwrap();
		assert_eq!(metadata.kind, EntryKind::File);
		assert_eq!(metadata.size, test_data.len() as u64);
		assert!(metadata.modified.is_some());
	}

	#[tokio::test]
	async fn test_local_backend_exists() {
		let temp_dir = TempDir::new().unwrap();
		let backend = LocalBackend::new(temp_dir.path());

		let test_path = Path::new("exists_test.txt");

		assert!(!backend.exists(test_path).await.unwrap());

		backend.write(test_path, Bytes::from("test")).await.unwrap();

		assert!(backend.exists(test_path).await.unwrap());
	}
}
