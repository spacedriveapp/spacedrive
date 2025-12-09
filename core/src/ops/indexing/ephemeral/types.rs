//! Core types for efficient ephemeral index storage
//!
//! This module provides compact data structures for storing file system entries
//! with minimal memory overhead. Key optimizations:
//! - 32-bit node IDs (4 bytes vs 8 bytes for u64)
//! - Bit-packed metadata (16 bytes for state, type, size, mtime, ctime)
//! - String interning via NameRef pointers
//!
//! Memory per node: ~48 bytes vs ~200 bytes with HashMap<PathBuf, EntryMetadata>

use smallvec::SmallVec;
use std::time::{SystemTime, UNIX_EPOCH};

/// Identifies a node in the arena. Uses u32 to halve memory vs u64
/// while supporting up to 4.3 billion nodes.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EntryId(u32);

impl EntryId {
	/// Create an EntryId from a usize index
	///
	/// # Panics
	/// Panics if index >= u32::MAX - 1 (reserved for NONE sentinel)
	pub fn from_usize(index: usize) -> Self {
		assert!(
			index < u32::MAX as usize - 1,
			"EntryId overflow: index {} exceeds maximum",
			index
		);
		Self(index as u32)
	}

	/// Get the underlying index as usize
	pub fn as_usize(self) -> usize {
		self.0 as usize
	}

	/// Get the raw u32 value
	pub fn as_u32(self) -> u32 {
		self.0
	}
}

/// Optional EntryId using u32::MAX as None sentinel
/// This saves 8 bytes per optional reference vs Option<EntryId>
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MaybeEntryId(u32);

impl MaybeEntryId {
	/// The sentinel value representing None
	pub const NONE: Self = Self(u32::MAX);

	/// Create a Some variant
	pub fn some(id: EntryId) -> Self {
		debug_assert!(id.0 != u32::MAX, "EntryId cannot use reserved NONE value");
		Self(id.0)
	}

	/// Convert to Option<EntryId>
	pub fn as_option(self) -> Option<EntryId> {
		if self.0 == u32::MAX {
			None
		} else {
			Some(EntryId(self.0))
		}
	}

	/// Check if this is None
	pub fn is_none(self) -> bool {
		self.0 == u32::MAX
	}

	/// Check if this is Some
	pub fn is_some(self) -> bool {
		self.0 != u32::MAX
	}
}

impl Default for MaybeEntryId {
	fn default() -> Self {
		Self::NONE
	}
}

impl From<Option<EntryId>> for MaybeEntryId {
	fn from(opt: Option<EntryId>) -> Self {
		match opt {
			Some(id) => Self::some(id),
			None => Self::NONE,
		}
	}
}

/// Node state indicating accessibility
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum NodeState {
	#[default]
	Unknown = 0,
	Accessible = 1,
	Inaccessible = 2,
}

impl NodeState {
	pub fn from_u8(value: u8) -> Self {
		match value {
			0 => Self::Unknown,
			1 => Self::Accessible,
			2 => Self::Inaccessible,
			_ => Self::Unknown,
		}
	}
}

/// File type classification
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FileType {
	#[default]
	Unknown = 0,
	File = 1,
	Directory = 2,
	Symlink = 3,
}

impl FileType {
	pub fn from_u8(value: u8) -> Self {
		match value {
			0 => Self::Unknown,
			1 => Self::File,
			2 => Self::Directory,
			3 => Self::Symlink,
			_ => Self::Unknown,
		}
	}
}

/// Convert from state::EntryKind to FileType
impl From<super::super::state::EntryKind> for FileType {
	fn from(kind: super::super::state::EntryKind) -> Self {
		match kind {
			super::super::state::EntryKind::File => FileType::File,
			super::super::state::EntryKind::Directory => FileType::Directory,
			super::super::state::EntryKind::Symlink => FileType::Symlink,
		}
	}
}

/// Convert from FileType to state::EntryKind
impl From<FileType> for super::super::state::EntryKind {
	fn from(ft: FileType) -> Self {
		match ft {
			FileType::File => super::super::state::EntryKind::File,
			FileType::Directory => super::super::state::EntryKind::Directory,
			FileType::Symlink => super::super::state::EntryKind::Symlink,
			FileType::Unknown => super::super::state::EntryKind::File, // Default to file
		}
	}
}

/// Compact metadata packed into 16 bytes
///
/// Layout:
/// - Bits 62-63: state (2 bits)
/// - Bits 60-61: type (2 bits)
/// - Bits 0-59: size (60 bits, max ~1 exabyte)
/// - mtime: seconds since epoch (32 bits)
/// - ctime: seconds since epoch (32 bits)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PackedMetadata {
	/// Bits 62-63: state, 60-61: type, 0-59: size
	state_type_size: u64,
	/// Modified time (seconds since epoch, 0 = None)
	mtime: u32,
	/// Created time (seconds since epoch, 0 = None)
	ctime: u32,
}

impl PackedMetadata {
	const SIZE_MASK: u64 = (1u64 << 60) - 1;
	const TYPE_SHIFT: u32 = 60;
	const STATE_SHIFT: u32 = 62;

	/// Create new packed metadata
	pub fn new(state: NodeState, file_type: FileType, size: u64) -> Self {
		// Clamp size to 60 bits (max ~1 exabyte)
		let size = size.min(Self::SIZE_MASK);
		let packed =
			size | ((file_type as u64) << Self::TYPE_SHIFT) | ((state as u64) << Self::STATE_SHIFT);

		Self {
			state_type_size: packed,
			mtime: 0,
			ctime: 0,
		}
	}

	/// Get the file size
	pub fn size(&self) -> u64 {
		self.state_type_size & Self::SIZE_MASK
	}

	/// Get the file type
	pub fn file_type(&self) -> FileType {
		FileType::from_u8(((self.state_type_size >> Self::TYPE_SHIFT) & 0b11) as u8)
	}

	/// Get the node state
	pub fn state(&self) -> NodeState {
		NodeState::from_u8(((self.state_type_size >> Self::STATE_SHIFT) & 0b11) as u8)
	}

	/// Set timestamps
	pub fn with_times(mut self, mtime: Option<SystemTime>, ctime: Option<SystemTime>) -> Self {
		self.mtime = mtime
			.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
			.map(|d| d.as_secs() as u32)
			.unwrap_or(0);
		self.ctime = ctime
			.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
			.map(|d| d.as_secs() as u32)
			.unwrap_or(0);
		self
	}

	/// Get modified time as SystemTime
	pub fn mtime_as_system_time(&self) -> Option<SystemTime> {
		if self.mtime == 0 {
			None
		} else {
			Some(UNIX_EPOCH + std::time::Duration::from_secs(self.mtime as u64))
		}
	}

	/// Get created time as SystemTime
	pub fn ctime_as_system_time(&self) -> Option<SystemTime> {
		if self.ctime == 0 {
			None
		} else {
			Some(UNIX_EPOCH + std::time::Duration::from_secs(self.ctime as u64))
		}
	}

	/// Get raw mtime value
	pub fn mtime_secs(&self) -> u32 {
		self.mtime
	}

	/// Get raw ctime value
	pub fn ctime_secs(&self) -> u32 {
		self.ctime
	}
}

impl Default for PackedMetadata {
	fn default() -> Self {
		Self::new(NodeState::Unknown, FileType::Unknown, 0)
	}
}

/// Reference to an interned string with parent link
///
/// Memory layout: 16 bytes total
/// - ptr: 8 bytes (pointer to string in NameCache)
/// - len: 4 bytes (string length)
/// - parent: 4 bytes (parent EntryId or NONE)
#[repr(C)]
pub struct NameRef {
	/// Pointer to string in NameCache (stable reference)
	ptr: *const u8,
	/// String length
	len: u32,
	/// Parent node ID (u32::MAX if root)
	parent: MaybeEntryId,
}

// SAFETY: NameRef contains a raw pointer to an interned string that lives
// as long as the NameCache. The NameCache is owned by EphemeralIndex and
// never deallocates strings. This makes NameRef safe to send between threads.
unsafe impl Send for NameRef {}
unsafe impl Sync for NameRef {}

impl NameRef {
	/// Create a new NameRef from an interned string
	///
	/// # Safety
	/// The interned string must live as long as any NameRef referencing it.
	/// This is guaranteed when used with NameCache.
	pub fn new(interned: &str, parent: MaybeEntryId) -> Self {
		Self {
			ptr: interned.as_ptr(),
			len: interned.len() as u32,
			parent,
		}
	}

	/// Get the filename
	///
	/// # Safety
	/// Assumes the interned string is still valid. This is guaranteed
	/// when NameCache is not dropped before NameRef.
	pub fn name(&self) -> &str {
		unsafe {
			std::str::from_utf8_unchecked(std::slice::from_raw_parts(self.ptr, self.len as usize))
		}
	}

	/// Get the parent entry ID
	pub fn parent(&self) -> Option<EntryId> {
		self.parent.as_option()
	}
}

impl std::fmt::Debug for NameRef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("NameRef")
			.field("name", &self.name())
			.field("parent", &self.parent.as_option())
			.finish()
	}
}

/// Single node in the file tree
///
/// Memory: ~48 bytes total
/// - name_ref: 16 bytes
/// - children: 8-24 bytes (SmallVec with inline storage)
/// - meta: 16 bytes
pub struct FileNode {
	/// Interned filename + parent reference
	pub name_ref: NameRef,
	/// Child node IDs (directories only)
	/// SmallVec stores 0 elements inline (8 bytes), grows on heap when needed
	pub children: SmallVec<[EntryId; 0]>,
	/// Packed metadata
	pub meta: PackedMetadata,
}

impl FileNode {
	/// Create a new file node
	pub fn new(name_ref: NameRef, meta: PackedMetadata) -> Self {
		Self {
			name_ref,
			children: SmallVec::new(),
			meta,
		}
	}

	/// Get the filename
	pub fn name(&self) -> &str {
		self.name_ref.name()
	}

	/// Get the parent entry ID
	pub fn parent(&self) -> Option<EntryId> {
		self.name_ref.parent()
	}

	/// Check if this is a directory
	pub fn is_directory(&self) -> bool {
		self.meta.file_type() == FileType::Directory
	}

	/// Add a child (for directories) - checks for duplicates
	pub fn add_child(&mut self, child_id: EntryId) {
		// Prevent duplicate children
		if !self.children.contains(&child_id) {
			self.children.push(child_id);
		}
	}
}

impl std::fmt::Debug for FileNode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FileNode")
			.field("name", &self.name())
			.field("type", &self.meta.file_type())
			.field("size", &self.meta.size())
			.field("children", &self.children.len())
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_entry_id_roundtrip() {
		let id = EntryId::from_usize(42);
		assert_eq!(id.as_usize(), 42);
		assert_eq!(id.as_u32(), 42);
	}

	#[test]
	fn test_maybe_entry_id() {
		let none = MaybeEntryId::NONE;
		assert!(none.is_none());
		assert!(!none.is_some());
		assert_eq!(none.as_option(), None);

		let some = MaybeEntryId::some(EntryId::from_usize(42));
		assert!(!some.is_none());
		assert!(some.is_some());
		assert_eq!(some.as_option(), Some(EntryId::from_usize(42)));
	}

	#[test]
	fn test_packed_metadata_size() {
		// Verify struct size is 16 bytes
		assert_eq!(std::mem::size_of::<PackedMetadata>(), 16);
	}

	#[test]
	fn test_packed_metadata_roundtrip() {
		let meta = PackedMetadata::new(NodeState::Accessible, FileType::File, 12345);

		assert_eq!(meta.state(), NodeState::Accessible);
		assert_eq!(meta.file_type(), FileType::File);
		assert_eq!(meta.size(), 12345);
	}

	#[test]
	fn test_packed_metadata_max_size() {
		// Test that large sizes are clamped
		let meta = PackedMetadata::new(NodeState::Accessible, FileType::File, u64::MAX);

		// Size should be clamped to 60-bit max
		assert_eq!(meta.size(), (1u64 << 60) - 1);
		assert_eq!(meta.file_type(), FileType::File);
	}

	#[test]
	fn test_packed_metadata_times() {
		use std::time::Duration;

		let mtime = UNIX_EPOCH + Duration::from_secs(1700000000);
		let ctime = UNIX_EPOCH + Duration::from_secs(1600000000);

		let meta = PackedMetadata::new(NodeState::Accessible, FileType::File, 1000)
			.with_times(Some(mtime), Some(ctime));

		assert_eq!(meta.mtime_secs(), 1700000000);
		assert_eq!(meta.ctime_secs(), 1600000000);
		assert!(meta.mtime_as_system_time().is_some());
		assert!(meta.ctime_as_system_time().is_some());
	}

	#[test]
	fn test_name_ref_size() {
		// Verify NameRef is 16 bytes
		assert_eq!(std::mem::size_of::<NameRef>(), 16);
	}

	#[test]
	fn test_file_type_conversion() {
		use crate::ops::indexing::state::EntryKind;

		assert_eq!(FileType::from(EntryKind::File), FileType::File);
		assert_eq!(FileType::from(EntryKind::Directory), FileType::Directory);
		assert_eq!(FileType::from(EntryKind::Symlink), FileType::Symlink);

		assert_eq!(EntryKind::from(FileType::File), EntryKind::File);
		assert_eq!(EntryKind::from(FileType::Directory), EntryKind::Directory);
		assert_eq!(EntryKind::from(FileType::Symlink), EntryKind::Symlink);
	}
}

