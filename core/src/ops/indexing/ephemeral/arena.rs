//! # Memory-Mapped Arena for Ephemeral File Nodes
//!
//! `NodeArena` stores file nodes in memory-mapped temporary files, allowing the OS
//! to page data in and out as needed. This prevents out-of-memory errors when browsing
//! large network shares or external drives with millions of files.
//!
//! Entries are stored contiguously at stable u32 indices (EntryIds), providing O(1)
//! lookup while keeping memory usage bounded. When RAM is tight, the OS pages cold
//! entries to disk automatically. The backing file is anonymous and cleaned up on drop,
//! so no manual file management is needed.
//!
//! The arena doubles capacity (1024 → 2048 → 4096 → ...) when full, minimizing
//! expensive remap operations while staying within Vec-like amortized O(1) insertion.

use super::types::{EntryId, FileNode};
use memmap2::{MmapMut, MmapOptions};
use std::{
	io,
	mem::{self, MaybeUninit},
	num::NonZeroUsize,
	slice,
};
use tempfile::NamedTempFile;

const CAPACITY: usize = 1024;

/// Slab allocator backed by an anonymous memory-mapped temporary file.
///
/// The OS manages paging, allowing large indexes to spill to disk under memory
/// pressure without crashing. EntryIds remain stable across capacity growth,
/// enabling parent-child relationships to persist through remaps.
pub struct NodeArena {
	file: NamedTempFile,
	mmap: MmapMut,
	capacity: NonZeroUsize,
	len: usize,
}

impl NodeArena {
	pub fn new() -> io::Result<Self> {
		Self::with_capacity(CAPACITY)
	}

	pub fn with_capacity(capacity: usize) -> io::Result<Self> {
		let capacity = NonZeroUsize::new(capacity.max(1)).unwrap();
		let mut file = NamedTempFile::new()?;
		let mmap = Self::map_file(&mut file, capacity)?;

		Ok(Self {
			file,
			mmap,
			capacity,
			len: 0,
		})
	}

	fn map_file(file: &mut NamedTempFile, slots: NonZeroUsize) -> io::Result<MmapMut> {
		let bytes = (slots.get() as u64).saturating_mul(mem::size_of::<FileNode>() as u64);
		file.as_file_mut().set_len(bytes)?;
		unsafe { MmapOptions::new().map_mut(file.as_file()) }
	}

	/// Doubles capacity until min_capacity is reached.
	fn ensure_capacity(&mut self, min_capacity: NonZeroUsize) -> io::Result<()> {
		if min_capacity <= self.capacity {
			return Ok(());
		}

		let mut new_capacity = self.capacity;
		while new_capacity < min_capacity {
			new_capacity = new_capacity.saturating_mul(NonZeroUsize::new(2).unwrap());
		}

		self.remap(new_capacity)
	}

	/// Flushes dirty pages, expands the file, and remaps with new capacity.
	fn remap(&mut self, new_capacity: NonZeroUsize) -> io::Result<()> {
		assert!(new_capacity.get() >= self.len);
		self.mmap.flush()?;
		self.mmap = Self::map_file(&mut self.file, new_capacity)?;
		self.capacity = new_capacity;
		Ok(())
	}

	fn grow(&mut self) -> io::Result<()> {
		let desired = self.capacity.saturating_mul(NonZeroUsize::new(2).unwrap());
		self.ensure_capacity(desired)
	}

	fn entries(&self) -> &[MaybeUninit<FileNode>] {
		unsafe {
			slice::from_raw_parts(
				self.mmap.as_ptr().cast::<MaybeUninit<FileNode>>(),
				self.capacity.get(),
			)
		}
	}

	fn entries_mut(&mut self) -> &mut [MaybeUninit<FileNode>] {
		unsafe {
			slice::from_raw_parts_mut(
				self.mmap.as_mut_ptr().cast::<MaybeUninit<FileNode>>(),
				self.capacity.get(),
			)
		}
	}

	/// Appends a node and returns its stable ID.
	///
	/// The arena grows automatically when full, remapping to a larger capacity.
	/// EntryIds remain valid across remaps since they're just indices.
	pub fn insert(&mut self, node: FileNode) -> io::Result<EntryId> {
		if self.len == self.capacity.get() {
			self.grow()?;
		}

		let index = self.len;
		let id = EntryId::from_usize(index);

		unsafe {
			self.entries_mut().get_unchecked_mut(index).write(node);
		}

		self.len += 1;
		Ok(id)
	}

	pub fn get(&self, id: EntryId) -> Option<&FileNode> {
		if id.as_usize() < self.len {
			Some(unsafe {
				self.entries()
					.get_unchecked(id.as_usize())
					.assume_init_ref()
			})
		} else {
			None
		}
	}

	pub fn get_mut(&mut self, id: EntryId) -> Option<&mut FileNode> {
		if id.as_usize() < self.len {
			Some(unsafe {
				self.entries_mut()
					.get_unchecked_mut(id.as_usize())
					.assume_init_mut()
			})
		} else {
			None
		}
	}

	pub fn len(&self) -> usize {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	/// No-op for memory-mapped arenas; the OS manages paging.
	pub fn shrink_to_fit(&mut self) {}

	pub fn capacity(&self) -> usize {
		self.capacity.get()
	}

	pub fn reserve(&mut self, additional: usize) -> io::Result<()> {
		let new_capacity = self.len.saturating_add(additional);
		if let Some(min_cap) = NonZeroUsize::new(new_capacity) {
			self.ensure_capacity(min_cap)?;
		}
		Ok(())
	}

	pub fn iter(&self) -> impl Iterator<Item = (EntryId, &FileNode)> {
		(0..self.len).map(move |i| {
			let id = EntryId::from_usize(i);
			let node = unsafe { self.entries().get_unchecked(i).assume_init_ref() };
			(id, node)
		})
	}

	pub fn iter_mut(&mut self) -> ArenaIterMut<'_> {
		let len = self.len;
		ArenaIterMut {
			entries: self.entries_mut(),
			len,
			index: 0,
		}
	}

	/// Reports total allocation including mmap overhead and child vectors.
	pub fn memory_usage(&self) -> usize {
		mem::size_of::<Self>()
			+ (self.capacity.get() * mem::size_of::<FileNode>())
			+ (0..self.len)
				.filter_map(|i| self.get(EntryId::from_usize(i)))
				.map(|n| n.children.capacity() * mem::size_of::<EntryId>())
				.sum::<usize>()
	}
}

pub struct ArenaIterMut<'a> {
	entries: &'a mut [MaybeUninit<FileNode>],
	len: usize,
	index: usize,
}

impl<'a> Iterator for ArenaIterMut<'a> {
	type Item = (EntryId, &'a mut FileNode);

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.len {
			return None;
		}

		let id = EntryId::from_usize(self.index);
		let node = unsafe {
			let ptr = self.entries.as_mut_ptr().add(self.index);
			&mut *(*ptr).as_mut_ptr()
		};

		self.index += 1;
		Some((id, node))
	}
}

impl Default for NodeArena {
	fn default() -> Self {
		Self::new().expect("Failed to create default NodeArena")
	}
}

impl Drop for NodeArena {
	fn drop(&mut self) {
		for i in 0..self.len {
			unsafe {
				self.entries_mut().get_unchecked_mut(i).assume_init_drop();
			}
		}

		let _ = self.mmap.flush();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::ops::indexing::ephemeral::types::{
		FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata,
	};

	fn make_test_node(name: &'static str) -> FileNode {
		let meta = PackedMetadata::new(NodeState::Accessible, FileType::File, 100);
		FileNode::new(NameRef::new(name, MaybeEntryId::NONE), meta)
	}

	#[test]
	fn test_insert_and_get() {
		let mut arena = NodeArena::new().expect("failed to create arena");

		let id1 = arena
			.insert(make_test_node("file1.txt"))
			.expect("insert failed");
		let id2 = arena
			.insert(make_test_node("file2.txt"))
			.expect("insert failed");

		assert_eq!(arena.len(), 2);
		assert_eq!(arena.get(id1).unwrap().name(), "file1.txt");
		assert_eq!(arena.get(id2).unwrap().name(), "file2.txt");
	}

	#[test]
	fn test_get_nonexistent() {
		let arena = NodeArena::new().expect("failed to create arena");
		assert!(arena.get(EntryId::from_usize(0)).is_none());
	}

	#[test]
	fn test_iteration() {
		let mut arena = NodeArena::new().expect("failed to create arena");

		arena.insert(make_test_node("a")).expect("insert failed");
		arena.insert(make_test_node("b")).expect("insert failed");
		arena.insert(make_test_node("c")).expect("insert failed");

		let names: Vec<&str> = arena.iter().map(|(_, node)| node.name()).collect();
		assert_eq!(names, vec!["a", "b", "c"]);
	}

	#[test]
	fn test_with_capacity() {
		let arena = NodeArena::with_capacity(1000).expect("failed to create arena");
		assert!(arena.capacity() >= 1000);
		assert!(arena.is_empty());
	}

	#[test]
	fn test_shrink_to_fit() {
		let mut arena = NodeArena::with_capacity(1000).expect("failed to create arena");
		arena.insert(make_test_node("a")).expect("insert failed");
		arena.shrink_to_fit();
		assert!(arena.capacity() >= 1000);
	}

	#[test]
	fn test_large_arena_growth() {
		let mut arena = NodeArena::new().expect("failed to create arena");

		// Pre-generate names so they have a stable address
		let names: Vec<String> = (0..10_000).map(|i| format!("file{}.txt", i)).collect();
		let static_names: Vec<&'static str> = names
			.iter()
			.map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
			.collect();

		for name in &static_names {
			let node = make_test_node(name);
			arena.insert(node).expect("insert should succeed");
		}

		assert_eq!(arena.len(), 10_000);
		assert!(arena.capacity() >= 10_000);

		for (i, name) in static_names.iter().enumerate() {
			let id = EntryId::from_usize(i);
			let node = arena.get(id).expect("node should exist");
			assert_eq!(node.name(), *name);
		}
	}
}
