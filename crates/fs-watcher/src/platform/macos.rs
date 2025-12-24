//! macOS-specific event handler
//!
//! macOS FSEvents doesn't provide native rename tracking. When a file is renamed,
//! we receive separate create and delete events. This handler implements rename
//! detection by tracking inodes and buffering events.
//!
//! Key features:
//! - Inode-based rename detection for both files and directories
//! - Three-phase event buffering (creates, updates, removes)
//! - Timeout-based eviction for unmatched events
//! - Finder duplicate directory event deduplication
//! - Reincident file tracking for files with rapid successive changes
//! - Buffered emission for rename detection

use crate::event::{FsEvent, RawEventKind, RawNotifyEvent};
use crate::platform::EventHandler;
use crate::Result;
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Timeout for rename detection buffering (matching old path with new path)
const RENAME_TIMEOUT_MS: u64 = 500;

/// Timeout for file stabilization (avoid processing mid-write files)
const STABILIZATION_TIMEOUT_MS: u64 = 500;

/// Longer timeout for files with rapid successive changes
const REINCIDENT_TIMEOUT_MS: u64 = 10_000;

/// Timeout for directory dedup cache (how long to remember recent directory creates)
const DIR_DEDUP_TIMEOUT_MS: u64 = 5_000;

/// macOS event handler with rename detection
pub struct MacOsHandler {
	/// Files pending potential rename (by inode) - the "old path" side
	/// Key: inode, Value: (path, timestamp)
	pending_removes: RwLock<HashMap<u64, PendingRemove>>,

	/// Recently created files (by inode) for rename matching - the "new path" side
	/// Key: inode, Value: (path, timestamp)
	pending_creates: RwLock<HashMap<u64, PendingCreate>>,

	/// Files to update after stabilization
	/// Key: path, Value: timestamp
	pending_updates: RwLock<HashMap<PathBuf, Instant>>,

	/// Files with multiple rapid changes - use longer timeout
	/// Key: path, Value: first change timestamp
	reincident_updates: RwLock<HashMap<PathBuf, Instant>>,

	/// Recently created directories - for duplicate event deduplication
	/// Key: path, Value: timestamp of creation
	recent_dirs: RwLock<HashMap<PathBuf, Instant>>,

	/// Inode cache for paths we've seen - allows rename detection even after file is moved
	/// Key: path, Value: (inode, timestamp)
	inode_cache: RwLock<HashMap<PathBuf, (u64, Instant)>>,
}

#[derive(Debug, Clone)]
struct PendingRemove {
	path: PathBuf,
	#[allow(dead_code)] // Used for debugging, will be useful for enhanced inode tracking
	inode: u64,
	timestamp: Instant,
}

#[derive(Debug, Clone)]
struct PendingCreate {
	path: PathBuf,
	inode: u64,
	timestamp: Instant,
}

impl MacOsHandler {
	/// Create a new macOS handler
	pub fn new() -> Self {
		Self {
			pending_removes: RwLock::new(HashMap::new()),
			pending_creates: RwLock::new(HashMap::new()),
			pending_updates: RwLock::new(HashMap::new()),
			reincident_updates: RwLock::new(HashMap::new()),
			recent_dirs: RwLock::new(HashMap::new()),
			inode_cache: RwLock::new(HashMap::new()),
		}
	}

	/// Get the inode for a path
	async fn get_inode(path: &PathBuf) -> Option<u64> {
		match tokio::fs::metadata(path).await {
			Ok(metadata) => Some(metadata.ino()),
			Err(_) => None,
		}
	}

	/// Check if path is a directory
	async fn is_directory(path: &PathBuf) -> bool {
		tokio::fs::metadata(path)
			.await
			.map(|m| m.is_dir())
			.unwrap_or(false)
	}

	/// Try to match a create event with a pending remove (rename detection)
	async fn try_match_rename(&self, path: &Path, inode: u64) -> Option<PathBuf> {
		let mut removes = self.pending_removes.write().await;
		if let Some(pending) = removes.remove(&inode) {
			debug!(
				"Rename detected: {} -> {} (inode {})",
				pending.path.display(),
				path.display(),
				inode
			);
			Some(pending.path)
		} else {
			None
		}
	}

	/// Process create events, attempting rename matching
	async fn process_create(&self, path: PathBuf) -> Result<Vec<FsEvent>> {
		let is_dir = Self::is_directory(&path).await;

		// Check if this is a directory and dedupe if needed
		if is_dir {
			let recent = self.recent_dirs.read().await;
			if recent.contains_key(&path) {
				trace!(
					"Ignoring duplicate directory create event: {}",
					path.display()
				);
				return Ok(vec![]);
			}
			// Note: Don't add to recent_dirs yet - only add when actually emitted
			// to avoid interfering with buffered rename detection
		}

		// Check if we already have this path in recent_dirs
		// (edge case: directory metadata check failed initially but file is actually a dir)
		{
			let recent = self.recent_dirs.read().await;
			if recent.contains_key(&path) {
				trace!(
					"Ignoring create event for recent directory: {}",
					path.display()
				);
				return Ok(vec![]);
			}
		}

		// Get inode for rename detection (works for both files and directories)
		let Some(inode) = Self::get_inode(&path).await else {
			// Path might have been deleted already
			debug!("Could not get inode for created path: {}", path.display());
			let event = if is_dir {
				FsEvent::create_dir(path)
			} else {
				FsEvent::create(path)
			};
			return Ok(vec![event]);
		};

		// Cache the inode for this path to enable rename detection
		{
			let mut cache = self.inode_cache.write().await;
			cache.insert(path.clone(), (inode, Instant::now()));
		}

		// Check if this matches a pending remove (rename)
		if let Some(from_path) = self.try_match_rename(&path, inode).await {
			let event = if is_dir {
				// Add to recent_dirs to prevent duplicate events for the renamed directory
				{
					let mut recent = self.recent_dirs.write().await;
					recent.insert(path.clone(), Instant::now());
				}
				FsEvent::rename_with_dir_flag(from_path, path, true)
			} else {
				FsEvent::rename(from_path, path)
			};
			return Ok(vec![event]);
		}

		// Buffer the create for potential later rename matching
		{
			let mut creates = self.pending_creates.write().await;
			creates.insert(
				inode,
				PendingCreate {
					path: path.clone(),
					inode,
					timestamp: Instant::now(),
				},
			);
		}

		// Don't emit yet - will be emitted on tick if no matching remove comes
		Ok(vec![])
	}

	/// Process remove events, buffering for rename detection
	async fn process_remove(&self, path: PathBuf) -> Result<Vec<FsEvent>> {
		// Try to get the inode from our pending creates (the file might already be gone)
		// If we can't get the inode, emit immediately as a remove
		let inode = {
			let creates = self.pending_creates.read().await;
			creates.values().find(|c| c.path == path).map(|c| c.inode)
		};

		// If we have a matching pending create, this is a rapid create+delete
		if let Some(inode) = inode {
			let mut creates = self.pending_creates.write().await;
			if creates.remove(&inode).is_some() {
				debug!(
					"Rapid create+delete detected, neutralizing: {}",
					path.display()
				);
				return Ok(vec![]);
			}
		}

		// Try to get inode from the filesystem (file might still exist briefly)
		let inode = if let Some(inode) = Self::get_inode(&path).await {
			Some(inode)
		} else {
			// File is already gone, try to get inode from cache
			let cache = self.inode_cache.read().await;
			cache.get(&path).map(|(inode, _)| *inode)
		};

		if let Some(inode) = inode {
			// Buffer for potential rename matching
			let mut removes = self.pending_removes.write().await;
			removes.insert(
				inode,
				PendingRemove {
					path: path.clone(),
					inode,
					timestamp: Instant::now(),
				},
			);
			trace!(
				"Buffered remove for rename detection: {} (inode {})",
				path.display(),
				inode
			);
			return Ok(vec![]);
		}

		// File is gone and we couldn't get inode from filesystem or cache - emit remove
		debug!(
			"No inode found for remove event, emitting immediately: {}",
			path.display()
		);
		Ok(vec![FsEvent::remove(path)])
	}

	/// Process modify events with stabilization buffering
	async fn process_modify(&self, path: PathBuf) -> Result<Vec<FsEvent>> {
		let mut updates = self.pending_updates.write().await;
		let mut reincident = self.reincident_updates.write().await;

		// Check if this file is already pending - track as reincident
		if let Some(old_instant) = updates.insert(path.clone(), Instant::now()) {
			// File had a previous pending update - mark as reincident for longer timeout
			reincident.entry(path).or_insert(old_instant);
		}

		Ok(vec![])
	}

	/// Evict pending creates that have timed out
	async fn evict_creates(&self, timeout: Duration) -> Vec<FsEvent> {
		let mut events = Vec::new();
		let mut to_process = Vec::new();

		// Collect timed-out entries
		{
			let mut creates = self.pending_creates.write().await;
			let mut to_remove = Vec::new();

			for (inode, pending) in creates.iter() {
				if pending.timestamp.elapsed() > timeout {
					to_remove.push(*inode);
				}
			}

			for inode in to_remove {
				if let Some(pending) = creates.remove(&inode) {
					to_process.push(pending);
				}
			}
		}

		// Process evictions without holding the creates lock
		for pending in to_process {
			// Check if this path was already emitted as a directory
			// (handles race condition where directory got buffered initially)
			let skip = {
				let recent = self.recent_dirs.read().await;
				let found = recent.contains_key(&pending.path);
				if found {
					debug!(
						"Skipping eviction for already-emitted directory: {}",
						pending.path.display()
					);
				} else {
					debug!(
						"Path not in recent_dirs, will evict: {} (recent_dirs has {} entries)",
						pending.path.display(),
						recent.len()
					);
				}
				found
			};

			if skip {
				continue;
			}

			// Check if the path is actually a directory now
			let is_dir = Self::is_directory(&pending.path).await;
			if is_dir {
				// Add to recent_dirs to prevent future duplicates
				{
					let mut recent = self.recent_dirs.write().await;
					recent.insert(pending.path.clone(), Instant::now());
				}
				events.push(FsEvent::create_dir(pending.path.clone()));
				debug!(
					"Evicting create as directory (was buffered as file): {}",
					pending.path.display()
				);
			} else {
				events.push(FsEvent::create_file(pending.path.clone()));
				debug!("Evicting create as file: {}", pending.path.display());
			}
		}

		events
	}

	/// Evict pending removes that have timed out
	async fn evict_removes(&self, timeout: Duration) -> Vec<FsEvent> {
		let mut events = Vec::new();
		let mut removes = self.pending_removes.write().await;
		let mut to_remove = Vec::new();

		for (inode, pending) in removes.iter() {
			if pending.timestamp.elapsed() > timeout {
				to_remove.push(*inode);
				events.push(FsEvent::remove(pending.path.clone()));
				trace!(
					"Evicting remove (no matching create): {}",
					pending.path.display()
				);
			}
		}

		for inode in to_remove {
			removes.remove(&inode);
		}

		events
	}

	/// Evict pending updates that have stabilized
	async fn evict_updates(&self, timeout: Duration) -> Vec<FsEvent> {
		let mut events = Vec::new();
		let mut updates = self.pending_updates.write().await;
		let mut reincident = self.reincident_updates.write().await;
		let reincident_timeout = Duration::from_millis(REINCIDENT_TIMEOUT_MS);

		let mut to_remove = Vec::new();

		for (path, timestamp) in updates.iter() {
			// Check if this is a reincident file (use longer timeout)
			let effective_timeout = if reincident.contains_key(path) {
				reincident_timeout
			} else {
				timeout
			};

			if timestamp.elapsed() > effective_timeout {
				to_remove.push(path.clone());
				// Emit as Create for files - the responder will detect if it's an update via inode
				events.push(FsEvent::create_file(path.clone()));
				trace!(
					"Evicting update (stabilized after {}ms): {}",
					timestamp.elapsed().as_millis(),
					path.display()
				);
			}
		}

		for path in &to_remove {
			updates.remove(path);
			reincident.remove(path);
		}

		events
	}

	/// Clean up old entries from the recent directories cache
	async fn cleanup_recent_dirs(&self, timeout: Duration) {
		let mut recent = self.recent_dirs.write().await;
		recent.retain(|_, timestamp| timestamp.elapsed() < timeout);
	}

	/// Clean up old entries from inode cache
	async fn cleanup_inode_cache(&self, timeout: Duration) {
		let mut cache = self.inode_cache.write().await;
		let now = Instant::now();
		cache.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < timeout);
	}
}

impl Default for MacOsHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl EventHandler for MacOsHandler {
	async fn process(&self, event: RawNotifyEvent) -> Result<Vec<FsEvent>> {
		match event.kind {
			RawEventKind::Create => {
				let Some(path) = event.primary_path().cloned() else {
					return Ok(vec![]);
				};
				self.process_create(path).await
			}
			RawEventKind::Remove => {
				let Some(path) = event.primary_path().cloned() else {
					return Ok(vec![]);
				};
				self.process_remove(path).await
			}
			RawEventKind::Modify => {
				let Some(path) = event.primary_path().cloned() else {
					return Ok(vec![]);
				};
				self.process_modify(path).await
			}
			RawEventKind::Rename => {
				// macOS FSEvents provides rename events with both paths
				// paths[0] = from (old path), paths[1] = to (new path)
				if event.paths.len() >= 2 {
					let from = event.paths[0].clone();
					let to = event.paths[1].clone();
					debug!(
						"Rename event received: {} -> {}",
						from.display(),
						to.display()
					);
					let is_dir = Self::is_directory(&to).await;
					return Ok(vec![FsEvent::rename_with_dir_flag(from, to, is_dir)]);
				} else if let Some(path) = event.primary_path() {
					// Single path rename event - treat as create (file appeared) or remove (file gone)
					// Check if path exists to determine which
					if path.exists() {
						debug!("Rename event with single path (exists): {}", path.display());
						return self.process_create(path.clone()).await;
					} else {
						debug!("Rename event with single path (gone): {}", path.display());
						return self.process_remove(path.clone()).await;
					}
				}
				Ok(vec![])
			}
			RawEventKind::Other(ref kind) => {
				trace!("Ignoring unknown event kind: {}", kind);
				Ok(vec![])
			}
		}
	}

	async fn tick(&self) -> Result<Vec<FsEvent>> {
		let rename_timeout = Duration::from_millis(RENAME_TIMEOUT_MS);
		let stabilization_timeout = Duration::from_millis(STABILIZATION_TIMEOUT_MS);
		let dir_dedup_timeout = Duration::from_millis(DIR_DEDUP_TIMEOUT_MS);

		let mut events = Vec::new();

		// Evict in order: updates first, then creates, then removes
		// This ensures proper ordering for related events
		events.extend(self.evict_updates(stabilization_timeout).await);
		events.extend(self.evict_creates(rename_timeout).await);
		events.extend(self.evict_removes(rename_timeout).await);

		// Clean up old entries from recent_dirs cache
		self.cleanup_recent_dirs(dir_dedup_timeout).await;

		// Clean up old entries from inode cache (use same timeout as dir dedup)
		self.cleanup_inode_cache(dir_dedup_timeout).await;

		Ok(events)
	}

	async fn reset(&self) {
		self.pending_removes.write().await.clear();
		self.pending_creates.write().await.clear();
		self.pending_updates.write().await.clear();
		self.reincident_updates.write().await.clear();
		self.recent_dirs.write().await.clear();
		self.inode_cache.write().await.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	#[tokio::test]
	async fn test_handler_creation() {
		let handler = MacOsHandler::new();
		// Should start with empty buffers
		assert!(handler.pending_removes.read().await.is_empty());
		assert!(handler.pending_creates.read().await.is_empty());
		assert!(handler.pending_updates.read().await.is_empty());
		assert!(handler.reincident_updates.read().await.is_empty());
		assert!(handler.recent_dirs.read().await.is_empty());
		assert!(handler.inode_cache.read().await.is_empty());
	}

	#[tokio::test]
	async fn test_reset() {
		let handler = MacOsHandler::new();

		// Add some pending data
		{
			let mut updates = handler.pending_updates.write().await;
			updates.insert(PathBuf::from("/test"), Instant::now());
		}
		{
			let mut recent = handler.recent_dirs.write().await;
			recent.insert(PathBuf::from("/test/dir"), Instant::now());
		}

		// Reset should clear everything
		handler.reset().await;

		assert!(handler.pending_updates.read().await.is_empty());
		assert!(handler.recent_dirs.read().await.is_empty());
	}

	#[tokio::test]
	async fn test_reincident_tracking() {
		let handler = MacOsHandler::new();
		let path = PathBuf::from("/test/file.txt");

		// First modify - should not be reincident
		{
			let mut updates = handler.pending_updates.write().await;
			updates.insert(path.clone(), Instant::now());
		}
		assert!(handler.reincident_updates.read().await.is_empty());

		// Second modify - should mark as reincident
		handler.process_modify(path.clone()).await.unwrap();
		assert!(handler.reincident_updates.read().await.contains_key(&path));
	}
}
