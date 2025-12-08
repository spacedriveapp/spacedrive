//! Vec-based arena storage for file nodes
//!
//! The NodeArena provides efficient, contiguous storage for FileNodes.
//! Key features:
//! - O(1) insertion and lookup by EntryId
//! - Cache-friendly contiguous memory layout
//! - Iteration over all nodes
//!
//! For very large indexes (10M+ files), this could be upgraded to use
//! memory-mapped storage, but Vec is sufficient for most use cases.

use super::types::{EntryId, FileNode};

/// Arena storage for file nodes using a simple Vec
///
/// Nodes are stored contiguously in memory for cache-friendly access.
/// EntryIds are stable indexes into this Vec.
pub struct NodeArena {
	/// Vector of nodes
	nodes: Vec<FileNode>,
}

impl NodeArena {
	/// Create a new empty arena
	pub fn new() -> Self {
		Self { nodes: Vec::new() }
	}

	/// Create an arena with pre-allocated capacity
	pub fn with_capacity(capacity: usize) -> Self {
		Self {
			nodes: Vec::with_capacity(capacity),
		}
	}

	/// Insert a node and return its ID
	pub fn insert(&mut self, node: FileNode) -> EntryId {
		let id = EntryId::from_usize(self.nodes.len());
		self.nodes.push(node);
		id
	}

	/// Get node by ID
	pub fn get(&self, id: EntryId) -> Option<&FileNode> {
		self.nodes.get(id.as_usize())
	}

	/// Get mutable node by ID
	pub fn get_mut(&mut self, id: EntryId) -> Option<&mut FileNode> {
		self.nodes.get_mut(id.as_usize())
	}

	/// Get the number of nodes
	pub fn len(&self) -> usize {
		self.nodes.len()
	}

	/// Check if the arena is empty
	pub fn is_empty(&self) -> bool {
		self.nodes.is_empty()
	}

	/// Shrink capacity to fit current size
	pub fn shrink_to_fit(&mut self) {
		self.nodes.shrink_to_fit();
	}

	/// Get current capacity
	pub fn capacity(&self) -> usize {
		self.nodes.capacity()
	}

	/// Reserve additional capacity
	pub fn reserve(&mut self, additional: usize) {
		self.nodes.reserve(additional);
	}

	/// Iterate over all nodes
	pub fn iter(&self) -> impl Iterator<Item = (EntryId, &FileNode)> {
		self.nodes
			.iter()
			.enumerate()
			.map(|(i, node)| (EntryId::from_usize(i), node))
	}

	/// Iterate over all nodes mutably
	pub fn iter_mut(&mut self) -> impl Iterator<Item = (EntryId, &mut FileNode)> {
		self.nodes
			.iter_mut()
			.enumerate()
			.map(|(i, node)| (EntryId::from_usize(i), node))
	}

	/// Get approximate memory usage in bytes
	pub fn memory_usage(&self) -> usize {
		// Base struct size + Vec allocation
		std::mem::size_of::<Self>()
			+ self.nodes.capacity() * std::mem::size_of::<FileNode>()
			+ self
				.nodes
				.iter()
				.map(|n| n.children.capacity() * std::mem::size_of::<EntryId>())
				.sum::<usize>()
	}
}

impl Default for NodeArena {
	fn default() -> Self {
		Self::new()
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
		let mut arena = NodeArena::new();

		let id1 = arena.insert(make_test_node("file1.txt"));
		let id2 = arena.insert(make_test_node("file2.txt"));

		assert_eq!(arena.len(), 2);
		assert_eq!(arena.get(id1).unwrap().name(), "file1.txt");
		assert_eq!(arena.get(id2).unwrap().name(), "file2.txt");
	}

	#[test]
	fn test_get_nonexistent() {
		let arena = NodeArena::new();
		assert!(arena.get(EntryId::from_usize(0)).is_none());
	}

	#[test]
	fn test_iteration() {
		let mut arena = NodeArena::new();

		arena.insert(make_test_node("a"));
		arena.insert(make_test_node("b"));
		arena.insert(make_test_node("c"));

		let names: Vec<&str> = arena.iter().map(|(_, node)| node.name()).collect();
		assert_eq!(names, vec!["a", "b", "c"]);
	}

	#[test]
	fn test_with_capacity() {
		let arena = NodeArena::with_capacity(1000);
		assert!(arena.capacity() >= 1000);
		assert!(arena.is_empty());
	}

	#[test]
	fn test_shrink_to_fit() {
		let mut arena = NodeArena::with_capacity(1000);
		arena.insert(make_test_node("a"));
		arena.shrink_to_fit();
		assert!(arena.capacity() < 1000);
	}
}
