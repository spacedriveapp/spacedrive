use std::{
	collections::HashMap,
	io::{Read, Seek, SeekFrom, Write},
	path::Path,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAGIC: &[u8; 8] = b"SDMEMORY";
const VERSION: u32 = 1;
const HEADER_SIZE: u64 = 64;

#[derive(Error, Debug)]
pub enum ArchiveError {
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),

	#[error("Invalid magic bytes")]
	InvalidMagic,

	#[error("Unsupported version: {0}")]
	UnsupportedVersion(u32),

	#[error("File not found in archive: {0}")]
	FileNotFound(String),

	#[error("Serialization error: {0}")]
	Serialization(#[from] rmp_serde::encode::Error),

	#[error("Deserialization error: {0}")]
	Deserialization(#[from] rmp_serde::decode::Error),

	#[error("Corrupt index")]
	CorruptIndex,
}

pub type Result<T> = std::result::Result<T, ArchiveError>;

/// Entry in the file index
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEntry {
	/// Offset in file where data starts
	offset: u64,
	/// Size of data in bytes
	size: u64,
	/// Whether data is compressed
	compressed: bool,
	/// Deleted flag (for soft deletes)
	deleted: bool,
}

/// File index (stored at end of archive)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileIndex {
	/// Filename -> FileEntry
	files: HashMap<String, FileEntry>,
}

/// Custom archive format for memory files
///
/// Format:
/// - Fixed 64-byte header with magic, version, index offset
/// - Append-only data section with length-prefixed files
/// - MessagePack-encoded index at end
///
/// Updates:
/// - Append new files to end
/// - Update index with new offsets
/// - Rewrite header with updated index offset
pub struct MemoryArchive {
	file: std::fs::File,
	index: FileIndex,
	index_offset: u64,
}

impl MemoryArchive {
	/// Create new archive
	pub fn create(path: &Path) -> Result<Self> {
		let mut file = std::fs::OpenOptions::new()
			.read(true)
			.write(true)
			.create(true)
			.truncate(true)
			.open(path)?;

		// Write header
		file.write_all(MAGIC)?;
		file.write_all(&VERSION.to_le_bytes())?;
		file.write_all(&0u32.to_le_bytes())?; // Flags (reserved)
		file.write_all(&HEADER_SIZE.to_le_bytes())?; // Index offset (will update)
		file.write_all(&vec![0u8; 40])?; // Reserved space

		// Write empty index at position 64
		let index = FileIndex {
			files: HashMap::new(),
		};

		let index_bytes = rmp_serde::to_vec(&index)?;
		file.write_all(&index_bytes)?;

		let index_offset = HEADER_SIZE;

		Ok(Self {
			file,
			index,
			index_offset,
		})
	}

	/// Open existing archive
	pub fn open(path: &Path) -> Result<Self> {
		let mut file = std::fs::OpenOptions::new()
			.read(true)
			.write(true)
			.open(path)?;

		// Read and validate header
		let mut magic = [0u8; 8];
		file.read_exact(&mut magic)?;
		if &magic != MAGIC {
			return Err(ArchiveError::InvalidMagic);
		}

		let mut version_bytes = [0u8; 4];
		file.read_exact(&mut version_bytes)?;
		let version = u32::from_le_bytes(version_bytes);
		if version != VERSION {
			return Err(ArchiveError::UnsupportedVersion(version));
		}

		// Skip flags
		file.seek(SeekFrom::Current(4))?;

		// Read index offset
		let mut offset_bytes = [0u8; 8];
		file.read_exact(&mut offset_bytes)?;
		let index_offset = u64::from_le_bytes(offset_bytes);

		// Seek to index and read it
		file.seek(SeekFrom::Start(index_offset))?;
		let mut index_bytes = Vec::new();
		file.read_to_end(&mut index_bytes)?;

		let index: FileIndex = rmp_serde::from_slice(&index_bytes)
			.map_err(|_| ArchiveError::CorruptIndex)?;

		Ok(Self {
			file,
			index,
			index_offset,
		})
	}

	/// Add a file to the archive
	pub fn add_file(&mut self, name: &str, data: &[u8]) -> Result<()> {
		// Seek to current index position (append before index)
		self.file.seek(SeekFrom::Start(self.index_offset))?;

		let offset = self.index_offset;
		let size = data.len() as u64;

		// Write: [length: u64][data: bytes]
		self.file.write_all(&size.to_le_bytes())?;
		self.file.write_all(data)?;

		// Update index
		self.index.files.insert(
			name.to_string(),
			FileEntry {
				offset: offset + 8, // After length prefix
				size,
				compressed: false,
				deleted: false,
			},
		);

		// New index position
		self.index_offset = offset + 8 + size;

		// Write updated index
		self.write_index()?;

		Ok(())
	}

	/// Read a file from the archive
	pub fn read_file(&mut self, name: &str) -> Result<Vec<u8>> {
		let entry = self
			.index
			.files
			.get(name)
			.ok_or_else(|| ArchiveError::FileNotFound(name.to_string()))?;

		if entry.deleted {
			return Err(ArchiveError::FileNotFound(name.to_string()));
		}

		// Seek to file offset
		self.file.seek(SeekFrom::Start(entry.offset))?;

		// Read data
		let mut data = vec![0u8; entry.size as usize];
		self.file.read_exact(&mut data)?;

		Ok(data)
	}

	/// Update a file (appends new version)
	pub fn update_file(&mut self, name: &str, data: &[u8]) -> Result<()> {
		// Just append as new file (index will point to latest)
		self.add_file(name, data)
	}

	/// Delete a file (soft delete in index)
	pub fn delete_file(&mut self, name: &str) -> Result<()> {
		if let Some(entry) = self.index.files.get_mut(name) {
			entry.deleted = true;
			self.write_index()?;
		}
		Ok(())
	}

	/// List all files
	pub fn list_files(&self) -> Vec<String> {
		self.index
			.files
			.iter()
			.filter(|(_, entry)| !entry.deleted)
			.map(|(name, _)| name.clone())
			.collect()
	}

	/// Check if file exists
	pub fn contains(&self, name: &str) -> bool {
		self.index
			.files
			.get(name)
			.map(|e| !e.deleted)
			.unwrap_or(false)
	}

	/// Write index to end of file and update header
	fn write_index(&mut self) -> Result<()> {
		// Serialize index
		let index_bytes = rmp_serde::to_vec(&self.index)?;

		// Write at current index offset
		self.file.seek(SeekFrom::Start(self.index_offset))?;
		self.file.write_all(&index_bytes)?;

		// Truncate file (remove old index if it was longer)
		let new_end = self.index_offset + index_bytes.len() as u64;
		self.file.set_len(new_end)?;

		// Update header with new index offset
		self.file.seek(SeekFrom::Start(16))?; // Skip magic + version + flags
		self.file.write_all(&self.index_offset.to_le_bytes())?;

		self.file.flush()?;

		Ok(())
	}

	/// Get total archive size
	pub fn size(&mut self) -> Result<u64> {
		Ok(self.file.metadata()?.len())
	}

	/// Compact archive (remove deleted files)
	pub fn compact(&mut self) -> Result<()> {
		// TODO: Implement garbage collection
		// Would require rewriting entire file with only non-deleted entries
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::NamedTempFile;

	#[test]
	fn test_create_archive() {
		let temp_file = NamedTempFile::new().unwrap();
		let _archive = MemoryArchive::create(temp_file.path()).unwrap();

		// Verify magic bytes
		let mut file = std::fs::File::open(temp_file.path()).unwrap();
		let mut magic = [0u8; 8];
		file.read_exact(&mut magic).unwrap();
		assert_eq!(&magic, MAGIC);
	}

	#[test]
	fn test_add_and_read_file() {
		let temp_file = NamedTempFile::new().unwrap();
		let mut archive = MemoryArchive::create(temp_file.path()).unwrap();

		let test_data = b"Hello, Memory!";
		archive.add_file("test.txt", test_data).unwrap();

		let read_data = archive.read_file("test.txt").unwrap();
		assert_eq!(read_data, test_data);
	}

	#[test]
	fn test_update_file() {
		let temp_file = NamedTempFile::new().unwrap();
		let mut archive = MemoryArchive::create(temp_file.path()).unwrap();

		archive.add_file("test.txt", b"Version 1").unwrap();
		archive.update_file("test.txt", b"Version 2").unwrap();

		let data = archive.read_file("test.txt").unwrap();
		assert_eq!(data, b"Version 2");
	}

	#[test]
	fn test_list_files() {
		let temp_file = NamedTempFile::new().unwrap();
		let mut archive = MemoryArchive::create(temp_file.path()).unwrap();

		archive.add_file("file1.txt", b"data1").unwrap();
		archive.add_file("file2.txt", b"data2").unwrap();
		archive.add_file("file3.txt", b"data3").unwrap();

		let files = archive.list_files();
		assert_eq!(files.len(), 3);
		assert!(files.contains(&"file1.txt".to_string()));
	}

	#[test]
	fn test_delete_file() {
		let temp_file = NamedTempFile::new().unwrap();
		let mut archive = MemoryArchive::create(temp_file.path()).unwrap();

		archive.add_file("test.txt", b"data").unwrap();
		assert!(archive.contains("test.txt"));

		archive.delete_file("test.txt").unwrap();
		assert!(!archive.contains("test.txt"));

		let result = archive.read_file("test.txt");
		assert!(result.is_err());
	}

	#[test]
	fn test_reopen_archive() {
		let temp_file = NamedTempFile::new().unwrap();
		let path = temp_file.path().to_path_buf();

		{
			let mut archive = MemoryArchive::create(&path).unwrap();
			archive.add_file("persisted.txt", b"test data").unwrap();
		}

		// Reopen
		let mut archive = MemoryArchive::open(&path).unwrap();
		let data = archive.read_file("persisted.txt").unwrap();
		assert_eq!(data, b"test data");
	}

	#[test]
	fn test_multiple_updates() {
		let temp_file = NamedTempFile::new().unwrap();
		let mut archive = MemoryArchive::create(temp_file.path()).unwrap();

		// Add initial
		archive.add_file("metadata.msgpack", b"v1").unwrap();

		// Update multiple times
		archive.update_file("metadata.msgpack", b"v2").unwrap();
		archive.update_file("metadata.msgpack", b"v3").unwrap();
		archive.update_file("metadata.msgpack", b"v4").unwrap();

		// Should read latest
		let data = archive.read_file("metadata.msgpack").unwrap();
		assert_eq!(data, b"v4");

		// File should still be single file
		assert_eq!(archive.list_files().len(), 1);
	}
}
