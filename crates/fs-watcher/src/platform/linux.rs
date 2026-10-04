//! Linux-specific event handler
//!
//! inotify reports a move as a MOVED_FROM for the old path and, when the new
//! path is inside the watched tree, a MOVED_TO for the new path. The notify
//! crate pairs the two by cookie and emits a two-path rename event. A
//! MOVED_FROM that never finds its partner means the file left the watched
//! tree, which is a removal from the tree's point of view, and a lone MOVED_TO
//! means a file arrived from outside, which is a creation. Both halves are held
//! for a short pairing window so a complete rename is reported once.

use crate::event::{FsEvent, RawEventKind, RawNotifyEvent, RawRenameMode};
use crate::platform::EventHandler;
use crate::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, trace};

/// Timeout for event stabilization
const STABILIZATION_TIMEOUT_MS: u64 = 100;

/// How long a lone rename half waits for its partner before it becomes a
/// Remove (MOVED_FROM) or Create (MOVED_TO). The kernel emits both halves of
/// an in-tree rename together, so the window only has to cover event delivery.
const RENAME_TIMEOUT_MS: u64 = 200;

/// Linux event handler
pub struct LinuxHandler {
	/// Files pending stabilization
	pending_updates: RwLock<HashMap<PathBuf, Instant>>,
	/// MOVED_FROM paths waiting for a matching MOVED_TO
	pending_move_from: RwLock<HashMap<PathBuf, Instant>>,
	/// MOVED_TO paths waiting for a matching MOVED_FROM
	pending_move_to: RwLock<HashMap<PathBuf, Instant>>,
}

impl LinuxHandler {
	/// Create a new Linux handler
	pub fn new() -> Self {
		Self {
			pending_updates: RwLock::new(HashMap::new()),
			pending_move_from: RwLock::new(HashMap::new()),
			pending_move_to: RwLock::new(HashMap::new()),
		}
	}

	/// Evict pending updates that have stabilized
	async fn evict_updates(&self, timeout: Duration) -> Vec<FsEvent> {
		let mut events = Vec::new();
		let mut updates = self.pending_updates.write().await;
		let mut to_remove = Vec::new();

		for (path, timestamp) in updates.iter() {
			if timestamp.elapsed() > timeout {
				to_remove.push(path.clone());
				events.push(FsEvent::modify(path.clone()));
				trace!("Evicting update (stabilized): {}", path.display());
			}
		}

		for path in to_remove {
			updates.remove(&path);
		}

		events
	}

	/// Unpaired MOVED_FROM entries older than the window left the watched tree.
	async fn evict_move_from(&self, timeout: Duration) -> Vec<FsEvent> {
		let mut events = Vec::new();
		let mut pending = self.pending_move_from.write().await;
		let expired: Vec<PathBuf> = pending
			.iter()
			.filter(|(_, timestamp)| timestamp.elapsed() > timeout)
			.map(|(path, _)| path.clone())
			.collect();

		for path in expired {
			pending.remove(&path);
			debug!(
				"Move out of watched tree, emitting remove: {}",
				path.display()
			);
			events.push(FsEvent::remove(path));
		}

		events
	}

	/// Unpaired MOVED_TO entries older than the window arrived from outside.
	async fn evict_move_to(&self, timeout: Duration) -> Vec<FsEvent> {
		let mut events = Vec::new();
		let mut pending = self.pending_move_to.write().await;
		let expired: Vec<PathBuf> = pending
			.iter()
			.filter(|(_, timestamp)| timestamp.elapsed() > timeout)
			.map(|(path, _)| path.clone())
			.collect();

		for path in expired {
			pending.remove(&path);
			debug!(
				"Move into watched tree, emitting create: {}",
				path.display()
			);
			events.push(FsEvent::create(path));
		}

		events
	}
}

impl Default for LinuxHandler {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait::async_trait]
impl EventHandler for LinuxHandler {
	async fn process(&self, event: RawNotifyEvent) -> Result<Vec<FsEvent>> {
		let Some(path) = event.primary_path().cloned() else {
			return Ok(vec![]);
		};

		match event.kind {
			RawEventKind::Create => Ok(vec![FsEvent::create(path)]),
			RawEventKind::Remove => {
				// A buffered modify for a path that is gone would only be
				// reported after the removal and then point at nothing.
				self.pending_updates.write().await.remove(&path);
				Ok(vec![FsEvent::remove(path)])
			}
			RawEventKind::Modify => {
				// Buffer modifications for stabilization
				let mut updates = self.pending_updates.write().await;
				updates.insert(path, Instant::now());
				Ok(vec![])
			}
			RawEventKind::Rename(_) if event.paths.len() >= 2 => {
				let from = event.paths[0].clone();
				let to = event.paths[1].clone();
				// notify already reported both halves on their own; drop them so
				// the rename is not followed by a spurious Remove and Create.
				self.pending_move_from.write().await.remove(&from);
				self.pending_move_to.write().await.remove(&to);
				Ok(vec![FsEvent::rename(from, to)])
			}
			RawEventKind::Rename(RawRenameMode::From) => {
				self.pending_updates.write().await.remove(&path);
				self.pending_move_from
					.write()
					.await
					.insert(path, Instant::now());
				Ok(vec![])
			}
			RawEventKind::Rename(RawRenameMode::To) => {
				self.pending_move_to
					.write()
					.await
					.insert(path, Instant::now());
				Ok(vec![])
			}
			RawEventKind::Rename(_) => {
				// No direction from the platform: the path's existence decides.
				if path.exists() {
					Ok(vec![FsEvent::create(path)])
				} else {
					Ok(vec![FsEvent::remove(path)])
				}
			}
			RawEventKind::Other(ref kind) => {
				trace!("Ignoring unknown event kind: {}", kind);
				Ok(vec![])
			}
		}
	}

	async fn tick(&self) -> Result<Vec<FsEvent>> {
		let stabilization_timeout = Duration::from_millis(STABILIZATION_TIMEOUT_MS);
		let rename_timeout = Duration::from_millis(RENAME_TIMEOUT_MS);

		let mut events = self.evict_updates(stabilization_timeout).await;
		events.extend(self.evict_move_from(rename_timeout).await);
		events.extend(self.evict_move_to(rename_timeout).await);
		Ok(events)
	}

	async fn reset(&self) {
		self.pending_updates.write().await.clear();
		self.pending_move_from.write().await.clear();
		self.pending_move_to.write().await.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn raw(kind: RawEventKind, paths: &[&str]) -> RawNotifyEvent {
		RawNotifyEvent {
			kind,
			paths: paths.iter().map(PathBuf::from).collect(),
			timestamp: std::time::SystemTime::now(),
		}
	}

	#[tokio::test]
	async fn test_handler_creation() {
		let handler = LinuxHandler::new();
		assert!(handler.pending_updates.read().await.is_empty());
	}

	#[tokio::test]
	async fn test_create_event() {
		let handler = LinuxHandler::new();
		let events = handler
			.process(raw(RawEventKind::Create, &["/test/file.txt"]))
			.await
			.unwrap();
		assert_eq!(events.len(), 1);
		assert!(events[0].kind.is_create());
	}

	#[tokio::test]
	async fn test_remove_event() {
		let handler = LinuxHandler::new();
		let events = handler
			.process(raw(RawEventKind::Remove, &["/test/file.txt"]))
			.await
			.unwrap();
		assert_eq!(events.len(), 1);
		assert!(events[0].kind.is_remove());
	}

	#[tokio::test]
	async fn test_rename_event() {
		let handler = LinuxHandler::new();
		let events = handler
			.process(raw(
				RawEventKind::Rename(RawRenameMode::Both),
				&["/test/old.txt", "/test/new.txt"],
			))
			.await
			.unwrap();
		assert_eq!(events.len(), 1);
		assert!(events[0].kind.is_rename());
	}

	#[tokio::test]
	async fn test_paired_rename_emits_once() {
		let handler = LinuxHandler::new();
		let from = RawEventKind::Rename(RawRenameMode::From);
		let to = RawEventKind::Rename(RawRenameMode::To);
		let both = RawEventKind::Rename(RawRenameMode::Both);

		assert!(handler
			.process(raw(from, &["/test/old.txt"]))
			.await
			.unwrap()
			.is_empty());
		assert!(handler
			.process(raw(to, &["/test/new.txt"]))
			.await
			.unwrap()
			.is_empty());
		let events = handler
			.process(raw(both, &["/test/old.txt", "/test/new.txt"]))
			.await
			.unwrap();
		assert_eq!(events.len(), 1);
		assert!(events[0].kind.is_rename());

		tokio::time::sleep(Duration::from_millis(RENAME_TIMEOUT_MS + 50)).await;
		assert!(handler.tick().await.unwrap().is_empty());
	}

	#[tokio::test]
	async fn test_remove_drops_buffered_modify() {
		let handler = LinuxHandler::new();
		assert!(handler
			.process(raw(RawEventKind::Modify, &["/test/file.txt"]))
			.await
			.unwrap()
			.is_empty());
		let events = handler
			.process(raw(RawEventKind::Remove, &["/test/file.txt"]))
			.await
			.unwrap();
		assert_eq!(events.len(), 1);
		assert!(events[0].kind.is_remove());

		tokio::time::sleep(Duration::from_millis(STABILIZATION_TIMEOUT_MS + 50)).await;
		assert!(handler.tick().await.unwrap().is_empty());
	}

	#[tokio::test]
	async fn test_move_out_of_tree_becomes_remove() {
		let handler = LinuxHandler::new();
		let events = handler
			.process(raw(
				RawEventKind::Rename(RawRenameMode::From),
				&["/test/gone.txt"],
			))
			.await
			.unwrap();
		assert!(events.is_empty());
		assert!(handler.tick().await.unwrap().is_empty());

		tokio::time::sleep(Duration::from_millis(RENAME_TIMEOUT_MS + 50)).await;
		let events = handler.tick().await.unwrap();
		assert_eq!(events.len(), 1);
		assert!(events[0].kind.is_remove());
		assert_eq!(events[0].path, PathBuf::from("/test/gone.txt"));
	}

	#[tokio::test]
	async fn test_move_into_tree_becomes_create() {
		let handler = LinuxHandler::new();
		let events = handler
			.process(raw(
				RawEventKind::Rename(RawRenameMode::To),
				&["/test/arrived.txt"],
			))
			.await
			.unwrap();
		assert!(events.is_empty());

		tokio::time::sleep(Duration::from_millis(RENAME_TIMEOUT_MS + 50)).await;
		let events = handler.tick().await.unwrap();
		assert_eq!(events.len(), 1);
		assert!(events[0].kind.is_create());
	}
}
