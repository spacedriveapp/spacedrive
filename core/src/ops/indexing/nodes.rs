//! # Memory-Mapped Storage for Arena File Nodes
//!
//! `NodeArena` stores file nodes in memory-mapped temporary files, allowing the OS
//! to page data in and out as needed. This prevents out-of-memory errors when browsing
//! large network shares or external drives with millions of files.
//!
//! Entries are stored contiguously at stable u32 indices (EntryIds), providing O(1)
//! lookup while keeping memory usage bounded. When RAM is tight, the OS pages cold
//! entries out to the backing file, which lives in the system temp directory: on
//! disk on most hosts, in RAM where that directory is a tmpfs.
//!
//! The backing file has no name in the filesystem, so the kernel reclaims it when
//! the process exits, however it exits. A named file would be removed only by
//! `Drop`, which a killed daemon never runs.
//!
//! The arena doubles capacity (1024 → 2048 → 4096 → ...) when full, minimizing
//! expensive remap operations while staying within Vec-like amortized O(1) insertion.
//!
//! A vacated slot goes on a free list and the next insert takes it, so a tree
//! that is cleared and refilled reuses the slots it had instead of appending
//! behind them. Reuse is safe because `EntryId`s never leave the `Arena`: its
//! public API speaks paths and uuids, and `Arena::detach` strips every internal
//! reference (name registry, parent child list, lookup maps) before vacating.

use super::types::{EntryId, FileNode};
use memmap2::{MmapMut, MmapOptions};
use std::{
	fs::File,
	io,
	mem::{self, MaybeUninit},
	num::NonZeroUsize,
	slice,
};

const CAPACITY: usize = 1024;

/// Slab allocator backed by an anonymous memory-mapped temporary file.
///
/// The OS manages paging, allowing large indexes to spill to disk under memory
/// pressure without crashing. EntryIds remain stable across capacity growth,
/// enabling parent-child relationships to persist through remaps.
pub struct NodeArena {
	file: File,
	mmap: MmapMut,
	capacity: NonZeroUsize,
	len: usize,
	/// Slots below `len` whose node was vacated, reused by the next inserts.
	free: Vec<EntryId>,
	/// One flag per slot below `len`; `false` while the slot is on the free
	/// list, so a lookup through a stale id answers nothing rather than an
	/// empty node.
	occupied: Vec<bool>,
}

impl NodeArena {
	pub fn new() -> io::Result<Self> {
		Self::with_capacity(CAPACITY)
	}

	pub fn with_capacity(capacity: usize) -> io::Result<Self> {
		let capacity = NonZeroUsize::new(capacity.max(1)).unwrap();
		let file = tempfile::tempfile()?;
		let mmap = Self::map_file(&file, capacity)?;

		Ok(Self {
			file,
			mmap,
			capacity,
			len: 0,
			free: Vec::new(),
			occupied: Vec::new(),
		})
	}

	fn map_file(file: &File, slots: NonZeroUsize) -> io::Result<MmapMut> {
		let bytes = (slots.get() as u64).saturating_mul(mem::size_of::<FileNode>() as u64);
		file.set_len(bytes)?;
		unsafe { MmapOptions::new().map_mut(file) }
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
		self.mmap = Self::map_file(&self.file, new_capacity)?;
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

	/// Stores a node in a vacated slot when one exists, otherwise appends it,
	/// and returns the slot's ID.
	///
	/// The arena grows automatically when full, remapping to a larger capacity.
	/// EntryIds remain valid across remaps since they're just indices.
	pub fn insert(&mut self, node: FileNode) -> io::Result<EntryId> {
		if let Some(id) = self.free.pop() {
			let slot = id.as_usize();
			unsafe {
				*self.entries_mut().get_unchecked_mut(slot).assume_init_mut() = node;
			}
			self.occupied[slot] = true;
			return Ok(id);
		}

		if self.len == self.capacity.get() {
			self.grow()?;
		}

		let index = self.len;
		let id = EntryId::from_usize(index);

		unsafe {
			self.entries_mut().get_unchecked_mut(index).write(node);
		}

		self.len += 1;
		self.occupied.push(true);
		Ok(id)
	}

	/// Release a slot: drop the node's heap (its children `SmallVec`) and put
	/// the slot on the free list for the next insert.
	///
	/// An id that is already vacant, or past the end, is ignored, so a slot can
	/// never be queued for reuse twice.
	pub fn vacate(&mut self, id: EntryId) {
		let slot = id.as_usize();
		if slot >= self.len || !self.occupied[slot] {
			return;
		}
		unsafe {
			*self.entries_mut().get_unchecked_mut(slot).assume_init_mut() = FileNode::vacant();
		}
		self.occupied[slot] = false;
		self.free.push(id);
	}

	fn is_occupied(&self, id: EntryId) -> bool {
		self.occupied.get(id.as_usize()).copied().unwrap_or(false)
	}

	pub fn get(&self, id: EntryId) -> Option<&FileNode> {
		if self.is_occupied(id) {
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
		if self.is_occupied(id) {
			Some(unsafe {
				self.entries_mut()
					.get_unchecked_mut(id.as_usize())
					.assume_init_mut()
			})
		} else {
			None
		}
	}

	/// Slots handed out so far, vacant ones included: the high-water mark that
	/// the mapping and the restart snapshot used to carry.
	pub fn len(&self) -> usize {
		self.len
	}

	/// Slots holding a node.
	pub fn live(&self) -> usize {
		self.len - self.free.len()
	}

	/// Slots waiting on the free list.
	pub fn vacant(&self) -> usize {
		self.free.len()
	}

	pub fn is_empty(&self) -> bool {
		self.live() == 0
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

	/// Every live node in slot order; vacant slots are skipped.
	pub fn iter(&self) -> impl Iterator<Item = (EntryId, &FileNode)> {
		(0..self.len)
			.filter(move |&i| self.occupied[i])
			.map(move |i| {
				let id = EntryId::from_usize(i);
				let node = unsafe { self.entries().get_unchecked(i).assume_init_ref() };
				(id, node)
			})
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
	use crate::ops::indexing::types::{FileType, MaybeEntryId, NameRef, NodeState, PackedMetadata};

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

	#[cfg(unix)]
	#[test]
	fn test_backing_file_has_no_name() {
		use std::os::unix::fs::MetadataExt;

		let arena = NodeArena::new().expect("failed to create arena");
		let links = arena
			.file
			.metadata()
			.expect("backing file metadata")
			.nlink();
		assert_eq!(links, 0, "a named backing file outlives a killed daemon");
	}

	#[test]
	fn a_vacated_slot_is_reused_and_unreadable_until_then() {
		let mut arena = NodeArena::new().expect("failed to create arena");
		let a = arena.insert(make_test_node("a")).unwrap();
		let b = arena.insert(make_test_node("b")).unwrap();

		arena.vacate(a);
		assert!(arena.get(a).is_none(), "a stale id answers nothing");
		assert_eq!(arena.live(), 1);
		assert_eq!(arena.vacant(), 1);
		assert_eq!(arena.iter().map(|(id, _)| id).collect::<Vec<_>>(), vec![b]);

		// Vacating twice must not queue the slot twice.
		arena.vacate(a);
		assert_eq!(arena.vacant(), 1);

		let c = arena.insert(make_test_node("c")).unwrap();
		assert_eq!(c, a, "the freed slot is taken before the arena grows");
		assert_eq!(arena.len(), 2);
		assert_eq!(arena.vacant(), 0);
		assert_eq!(arena.get(c).unwrap().name(), "c");
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
