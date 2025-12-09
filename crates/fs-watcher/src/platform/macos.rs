//! macOS-specific event handler
//!
//! macOS FSEvents doesn't provide native rename tracking. When a file is renamed,
//! we receive separate create and delete events. This handler implements rename
//! detection by tracking inodes and buffering events.
//!
//! Key features:
//! - Inode-based rename detection
//! - Three-phase event buffering (creates, updates, removes)
//! - Timeout-based eviction for unmatched events
//! - Finder duplicate directory event deduplication
//! - Reincident file tracking for files with rapid successive changes
//! - Immediate emission for directories, buffered emission for files

use crate::event::{FsEvent, RawEventKind, RawNotifyEvent};
use crate::platform::EventHandler;
use crate::Result;
use std::collections::HashMap;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Timeout for rename detection buffering (matching old path with new path)
const RENAME_TIMEOUT_MS: u64 = 500;

/// Timeout for file stabilization (avoid processing mid-write files)
const STABILIZATION_TIMEOUT_MS: u64 = 500;

/// Longer timeout for files with rapid successive changes
const REINCIDENT_TIMEOUT_MS: u64 = 10_000;

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

	/// Last created directory path - for Finder duplicate event deduplication
	last_created_dir: RwLock<Option<PathBuf>>,
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
			last_created_dir: RwLock::new(None),
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
	async fn try_match_rename(&self, path: &PathBuf, inode: u64) -> Option<PathBuf> {
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
		// Check if this is a directory
		if Self::is_directory(&path).await {
			// Dedupe Finder's duplicate directory creation events
			{
				let mut last_dir = self.last_created_dir.write().await;
				if let Some(ref last) = *last_dir {
					if *last == path {
						trace!(
							"Ignoring duplicate directory create event: {}",
							path.display()
						);
						return Ok(vec![]);
					}
				}
				*last_dir = Some(path.clone());
			}

			// Directories emit immediately (no rename detection needed)
			debug!(
				"Directory created, emitting immediately: {}",
				path.display()
			);
			return Ok(vec![FsEvent::create_dir(path)]);
		}

		// For files, get inode for rename detection
		let Some(inode) = Self::get_inode(&path).await else {
			// File might have been deleted already
			debug!("Could not get inode for created file: {}", path.display());
			return Ok(vec![FsEvent::create(path)]);
		};

		// Check if this matches a pending remove (rename)
		if let Some(from_path) = self.try_match_rename(&path, inode).await {
			return Ok(vec![FsEvent::rename(from_path, path)]);
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
		if let Some(inode) = Self::get_inode(&path).await {
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
			trace!("Buffered remove for rename detection: {}", path.display());
			return Ok(vec![]);
		}

		// File is gone and we couldn't get inode - emit remove
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
		let mut creates = self.pending_creates.write().await;
		let mut to_remove = Vec::new();

		for (inode, pending) in creates.iter() {
			if pending.timestamp.elapsed() > timeout {
				to_remove.push(*inode);
				// Files only - directories are emitted immediately in process_create
				events.push(FsEvent::create_file(pending.path.clone()));
				trace!(
					"Evicting create (no matching remove): {}",
					pending.path.display()
				);
			}
		}

		for inode in to_remove {
			creates.remove(&inode);
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

		let mut events = Vec::new();

		// Evict in order: updates first, then creates, then removes
		// This ensures proper ordering for related events
		events.extend(self.evict_updates(stabilization_timeout).await);
		events.extend(self.evict_creates(rename_timeout).await);
		events.extend(self.evict_removes(rename_timeout).await);

		Ok(events)
	}

	async fn reset(&self) {
		self.pending_removes.write().await.clear();
		self.pending_creates.write().await.clear();
		self.pending_updates.write().await.clear();
		self.reincident_updates.write().await.clear();
		*self.last_created_dir.write().await = None;
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
		assert!(handler.last_created_dir.read().await.is_none());
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
			let mut last_dir = handler.last_created_dir.write().await;
			*last_dir = Some(PathBuf::from("/test/dir"));
		}

		// Reset should clear everything
		handler.reset().await;

		assert!(handler.pending_updates.read().await.is_empty());
		assert!(handler.last_created_dir.read().await.is_none());
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
