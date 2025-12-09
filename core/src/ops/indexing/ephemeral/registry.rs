//! Name-based lookup registry for fast queries
//!
//! The NameRegistry provides O(log k) lookups by filename across the entire index.
//! This enables efficient queries like "find all files named 'package.json'".
//!
//! Features:
//! - Fast exact name lookup: O(log k) where k = unique filenames
//! - Prefix search for autocomplete
//! - Multiple entries per name (common for files like 'index.js', 'README.md')

use super::types::EntryId;
use std::collections::BTreeMap;

/// Maps filenames to node IDs for fast name-based queries
///
/// Uses BTreeMap for ordered iteration and efficient prefix searches.
/// Each name can map to multiple EntryIds (e.g., many 'index.js' files).
pub struct NameRegistry {
	/// Maps interned name pointers to entry IDs
	/// Using *const str as key since we use interned strings from NameCache
	map: BTreeMap<NameKey, Vec<EntryId>>,
}

/// Key type for the registry that wraps an interned string pointer
#[derive(Clone, Copy, PartialEq, Eq)]
struct NameKey(*const str);

impl NameKey {
	fn as_str(&self) -> &str {
		// SAFETY: The pointer comes from NameCache and remains valid
		unsafe { &*self.0 }
	}
}

impl PartialOrd for NameKey {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for NameKey {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.as_str().cmp(other.as_str())
	}
}

// SAFETY: NameKey contains a pointer to an interned string that lives
// as long as the NameCache. Since NameCache is thread-safe and never
// deallocates, NameKey is safe to use across threads.
unsafe impl Send for NameKey {}
unsafe impl Sync for NameKey {}

impl NameRegistry {
	/// Create a new empty registry
	pub fn new() -> Self {
		Self {
			map: BTreeMap::new(),
		}
	}

	/// Register a name-to-entry mapping
	///
	/// # Arguments
	/// * `name` - An interned string reference from NameCache
	/// * `id` - The EntryId to associate with this name
	pub fn insert(&mut self, name: &str, id: EntryId) {
		let key = NameKey(name as *const str);
		self.map.entry(key).or_default().push(id);
	}

	/// Get all entries with the exact name
	pub fn get(&self, name: &str) -> Option<&[EntryId]> {
		// We need to find by string content, not pointer
		// This is less efficient but works with non-interned queries
		for (key, ids) in &self.map {
			if key.as_str() == name {
				return Some(ids.as_slice());
			}
		}
		None
	}

	/// Get all entries with the exact name (using interned pointer)
	///
	/// More efficient when you have an interned string
	pub fn get_interned(&self, name: &str) -> Option<&[EntryId]> {
		let key = NameKey(name as *const str);
		self.map.get(&key).map(|v| v.as_slice())
	}

	/// Find all entries with names starting with the given prefix
	///
	/// Useful for autocomplete and directory listings
	pub fn find_prefix(&self, prefix: &str) -> Vec<EntryId> {
		self.map
			.iter()
			.filter(|(k, _)| k.as_str().starts_with(prefix))
			.flat_map(|(_, ids)| ids.iter().copied())
			.collect()
	}

	/// Find all entries with names containing the given substring
	pub fn find_containing(&self, substring: &str) -> Vec<EntryId> {
		self.map
			.iter()
			.filter(|(k, _)| k.as_str().contains(substring))
			.flat_map(|(_, ids)| ids.iter().copied())
			.collect()
	}

	/// Get the number of unique names
	pub fn unique_names(&self) -> usize {
		self.map.len()
	}

	/// Get the total number of entries
	pub fn total_entries(&self) -> usize {
		self.map.values().map(|v| v.len()).sum()
	}

	/// Check if a name exists in the registry
	pub fn contains(&self, name: &str) -> bool {
		self.get(name).is_some()
	}

	/// Get approximate memory usage in bytes
	pub fn memory_usage(&self) -> usize {
		std::mem::size_of::<Self>()
			+ self.map.len() * std::mem::size_of::<(NameKey, Vec<EntryId>)>()
			+ self
				.map
				.values()
				.map(|v| v.capacity() * std::mem::size_of::<EntryId>())
				.sum::<usize>()
	}

	/// Iterate over all (name, entry_ids) pairs
	pub fn iter(&self) -> impl Iterator<Item = (&str, &[EntryId])> {
		self.map.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
	}
}

impl Default for NameRegistry {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_insert_and_get() {
		let mut registry = NameRegistry::new();

		let name = "test.txt";
		let id = EntryId::from_usize(42);

		registry.insert(name, id);

		let result = registry.get("test.txt");
		assert!(result.is_some());
		assert_eq!(result.unwrap(), &[id]);
	}

	#[test]
	fn test_multiple_entries_same_name() {
		let mut registry = NameRegistry::new();

		// Many projects have multiple index.js files
		let name = "index.js";
		let ids: Vec<EntryId> = (0..5).map(|i| EntryId::from_usize(i)).collect();

		for &id in &ids {
			registry.insert(name, id);
		}

		let result = registry.get("index.js").unwrap();
		assert_eq!(result.len(), 5);
	}

	#[test]
	fn test_find_prefix() {
		let mut registry = NameRegistry::new();

		registry.insert("README.md", EntryId::from_usize(1));
		registry.insert("README.txt", EntryId::from_usize(2));
		registry.insert("README", EntryId::from_usize(3));
		registry.insert("Rakefile", EntryId::from_usize(4));

		let results = registry.find_prefix("README");
		assert_eq!(results.len(), 3);
	}

	#[test]
	fn test_find_containing() {
		let mut registry = NameRegistry::new();

		registry.insert("my_test.rs", EntryId::from_usize(1));
		registry.insert("test_utils.rs", EntryId::from_usize(2));
		registry.insert("integration_test.rs", EntryId::from_usize(3));
		registry.insert("main.rs", EntryId::from_usize(4));

		let results = registry.find_containing("test");
		assert_eq!(results.len(), 3);
	}

	#[test]
	fn test_unique_names_vs_total() {
		let mut registry = NameRegistry::new();

		// 3 unique names, 6 total entries
		registry.insert("a.txt", EntryId::from_usize(1));
		registry.insert("a.txt", EntryId::from_usize(2));
		registry.insert("b.txt", EntryId::from_usize(3));
		registry.insert("b.txt", EntryId::from_usize(4));
		registry.insert("c.txt", EntryId::from_usize(5));
		registry.insert("c.txt", EntryId::from_usize(6));

		assert_eq!(registry.unique_names(), 3);
		assert_eq!(registry.total_entries(), 6);
	}
}
