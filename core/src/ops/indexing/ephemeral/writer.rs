//! The one path from a filesystem change to everything that has to hear about
//! it.
//!
//! Three destinations have to agree about every change: the arena, which is
//! what a listing reads; the source store, which is what survives a restart;
//! and the clients, which are what someone is looking at. Writing all three by
//! hand at each call site is how they drift, and they have: a move once updated
//! the arena and the store correctly and told nobody, because creates emitted
//! inline while moves went through a hook that was handed the path the file had
//! just left.
//!
//! So there is one entry point. [`ArenaWriter::apply`] takes what was seen,
//! resolves identity, mutates the arena, and answers with the change it made.
//! The store write and the client event both come out of that one description,
//! which is what makes "written durably but never announced" unrepresentable
//! rather than merely fixed.
//!
//! The walk and the watcher differ only in how much arrives at once and how
//! much anyone wants to hear about it, which is [`Notify`].

use crate::domain::ContentKind;
use crate::infra::event::{Event, EventBus};
use crate::ops::indexing::change_detection::handler::{build_dir_entry, ChangeHandler};
use crate::ops::indexing::change_detection::types::{ChangeType, EntryRef};
use crate::ops::indexing::metadata::{is_hidden_path, EntryMetadata};
use crate::ops::indexing::state::{DirEntry, EntryKind};

use super::store::SourceStore;
use super::types::Rollup;
use super::EphemeralIndex;

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// What the filesystem did, however it came to be known.
///
/// A walk that just read a directory and a watcher that was handed an event
/// describe the same things, so they describe them the same way.
pub enum Seen {
	/// Entries that are there. One from the watcher, a batch from a walk.
	Entries(Vec<EntryMetadata>),
	/// A rename with both ends known, so identity needs no guessing.
	Renamed { from: PathBuf, to: EntryMetadata },
	/// A path that is gone. A directory takes its subtree with it.
	Lost { path: PathBuf, is_directory: bool },
	/// A directory the walk counted rather than kept, and the totals standing
	/// in for its contents. Also how those totals are corrected later: a
	/// summary has no children to hear a change from, so it is counted again.
	Counted { path: PathBuf, totals: Rollup },
}

/// What the arena did about it.
///
/// Both the client event and the store write are derived from this, so a change
/// that reaches one reaches the other.
#[derive(Debug, Clone)]
pub struct ArenaChange {
	pub mutation: Mutation,
	/// The identity the arena holds, which is the one queries will resolve.
	pub uuid: Uuid,
	pub path: PathBuf,
	/// What the entry is now. Absent for a delete: there is nothing left to
	/// describe.
	pub metadata: Option<EntryMetadata>,
	pub content_kind: ContentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
	Created,
	Modified,
	/// The arena took the entry in and nothing about it moved. A walk over a
	/// drive that has been walked before is almost entirely these, and
	/// announcing them would put a million files nobody touched through
	/// serialisation on the way to a client that would render them identically.
	Unchanged,
	/// Carries where it came from, because a client renders directory listings
	/// and a move invalidates two of them.
	Moved {
		from: PathBuf,
	},
	Deleted,
	/// A summary's totals were set or corrected. Nothing durable moved, so this
	/// is the one mutation the store has no interest in.
	Counted,
}

/// What an observation did, against what the arena held for that path.
///
/// Size and mtime are what a walk can compare cheaply and what everything
/// downstream keys on: the thumbnail and block caches validate against them, so
/// a file whose pair is unchanged is a file nothing has to be told about.
///
/// Compared at whole seconds, which is the precision the arena keeps. A
/// filesystem hands out nanoseconds and the arena rounds them off, so comparing
/// what was read against what was stored would find every file different.
fn mutation_for(before: Option<&EntryMetadata>, now: &EntryMetadata) -> Mutation {
	fn seconds(time: Option<std::time::SystemTime>) -> Option<u64> {
		time.and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
			.map(|since| since.as_secs())
	}

	match before {
		None => Mutation::Created,
		Some(before)
			if before.size == now.size && seconds(before.modified) == seconds(now.modified) =>
		{
			Mutation::Unchanged
		}
		Some(_) => Mutation::Modified,
	}
}

impl ArenaChange {
	/// The directories whose listings this change invalidates.
	fn affected(&self) -> Vec<PathBuf> {
		let mut paths: Vec<PathBuf> = self
			.path
			.parent()
			.map(Path::to_path_buf)
			.into_iter()
			.collect();
		if let Mutation::Moved { from } = &self.mutation {
			paths.extend(from.parent().map(Path::to_path_buf));
		}
		paths
	}

	fn is_hidden(&self) -> bool {
		is_hidden_path(&self.path)
	}
}

/// How much the client hears about what the writer does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notify {
	/// Every change, as it happens. The watcher's shape: a handful of events,
	/// each one about something a person is looking at.
	Each,
	/// One event per batch. A walk of a folder someone opened, where the
	/// listing is rebuilt from what arrived rather than from every notice.
	Batched,
	/// Nothing. Mapping a drive nobody asked to see.
	Silent,
}

/// Writes a filesystem change to the arena, the store and the clients.
///
/// The arena is written inline because a browse is waiting on it. The store is
/// handed the same observation through a queue, so nothing on the read path
/// ever waits on SQLite. A partition with no store (scratch, or a cache with no
/// directory to write to) keeps the identity the arena has and skips the second
/// write.
pub struct ArenaWriter {
	index: Arc<RwLock<EphemeralIndex>>,
	event_bus: Arc<EventBus>,
	store: Option<Arc<SourceStore>>,
	notify: Notify,
}

impl ArenaWriter {
	/// A writer for the watcher: one change at a time, each one announced.
	pub fn new(
		index: Arc<RwLock<EphemeralIndex>>,
		event_bus: Arc<EventBus>,
		store: Option<Arc<SourceStore>>,
	) -> Self {
		Self {
			index,
			event_bus,
			store,
			notify: Notify::Each,
		}
	}

	/// How much to tell clients. A walk sets this; the watcher keeps the
	/// default.
	pub fn notifying(mut self, notify: Notify) -> Self {
		self.notify = notify;
		self
	}

	/// Apply what was seen: identity, then the arena, then everyone who has to
	/// hear about it.
	pub async fn apply(&self, seen: Seen) -> Vec<ArenaChange> {
		let changes = match seen {
			Seen::Entries(entries) => self.entries(entries).await,
			Seen::Renamed { from, to } => self.renamed(from, to).await,
			Seen::Lost { path, is_directory } => self.lost(path, is_directory).await,
			Seen::Counted { path, totals } => self.counted(path, totals).await,
		};

		self.tell_clients(&changes);
		changes
	}

	/// Entries the filesystem says are there.
	///
	/// Identity comes from the store's ledger, which is also where the batch
	/// gets taken in, so asking for an identity is the durable write. Where the
	/// arena already holds one the ledger is offered it and adopts it, which is
	/// what keeps a source created over an already-mapped drive from minting a
	/// second identity for every file on it.
	async fn entries(&self, entries: Vec<EntryMetadata>) -> Vec<ArenaChange> {
		if entries.is_empty() {
			return Vec::new();
		}

		// Read under one lock and released before the store is asked: the queue
		// round trip must not be taken while readers are waiting on the arena.
		let (known, before): (Vec<Option<Uuid>>, Vec<Option<EntryMetadata>>) = {
			let index = self.index.read().await;
			entries
				.iter()
				.map(|entry| {
					(
						index.get_entry_uuid(&entry.path),
						index.get_entry_ref(&entry.path),
					)
				})
				.unzip()
		};

		let identities = match &self.store {
			Some(store) => store.identify(&entries, &known).await,
			// No ledger to ask. A writer nobody is listening to leaves the uuid
			// to be assigned when something first refers to the entry, which is
			// what keeps a whole-drive map from carrying one per file. Anything
			// that announces has to mint now, because an event names the
			// identity a query will be asked for.
			None if self.notify == Notify::Silent => known.clone(),
			None => known
				.iter()
				.map(|known| Some(known.unwrap_or_else(Uuid::now_v7)))
				.collect(),
		};

		let batch: Vec<(PathBuf, Option<Uuid>, EntryMetadata)> = entries
			.iter()
			.cloned()
			.zip(identities)
			.map(|(entry, identity)| (entry.path.clone(), identity, entry))
			.collect();

		let mut index = self.index.write().await;
		let Ok(added) = index.add_entries_batch(batch) else {
			return Vec::new();
		};

		entries
			.into_iter()
			.zip(added)
			.zip(before)
			.map(|((entry, (content_kind, uuid)), before)| {
				// A path the arena already held answers with no content kind,
				// because it was classified when it first arrived.
				let content_kind =
					content_kind.unwrap_or_else(|| index.get_content_kind(&entry.path));
				ArenaChange {
					mutation: mutation_for(before.as_ref(), &entry),
					uuid,
					path: entry.path.clone(),
					metadata: Some(entry),
					content_kind,
				}
			})
			.collect()
	}

	/// A rename carries both ends, so the store rebinds rather than re-deriving
	/// identity from an inode it would have to trust.
	async fn renamed(&self, from: PathBuf, to: EntryMetadata) -> Vec<ArenaChange> {
		let uuid = {
			let index = self.index.read().await;
			index.get_entry_uuid(&from)
		};

		if let Some(store) = &self.store {
			store.renamed(&from, &to).await;
		}

		let uuid = uuid.unwrap_or_else(Uuid::now_v7);
		let path = to.path.clone();

		let content_kind = {
			let mut index = self.index.write().await;
			index.remove_entry(&from);
			if index.add_entry(path.clone(), uuid, to.clone()).is_err() {
				return Vec::new();
			}
			index.get_content_kind(&path)
		};

		vec![ArenaChange {
			mutation: Mutation::Moved { from },
			uuid,
			path,
			metadata: Some(to),
			content_kind,
		}]
	}

	async fn lost(&self, path: PathBuf, is_directory: bool) -> Vec<ArenaChange> {
		let uuid = {
			let index = self.index.read().await;
			index.get_entry_uuid(&path)
		};

		{
			let mut index = self.index.write().await;
			if is_directory {
				index.remove_directory_tree(&path);
			} else {
				index.remove_entry(&path);
			}
		}

		if let Some(store) = &self.store {
			store.lost(&path, is_directory).await;
		}

		let Some(uuid) = uuid else {
			// Nothing the arena knew about, so nothing anyone can be holding.
			return Vec::new();
		};

		vec![ArenaChange {
			mutation: Mutation::Deleted,
			uuid,
			path,
			metadata: None,
			content_kind: ContentKind::Unknown,
		}]
	}

	/// A summarised directory's totals.
	///
	/// The store holds no rollups, so there is nothing durable to tell. What
	/// changed is a size a client is drawing, which is the whole point of
	/// keeping the count honest.
	async fn counted(&self, path: PathBuf, totals: Rollup) -> Vec<ArenaChange> {
		let (uuid, metadata) = {
			let mut index = self.index.write().await;
			index.summarise(&path, totals);
			(index.get_entry_uuid(&path), index.get_entry(&path))
		};

		let Some(uuid) = uuid else {
			return Vec::new();
		};

		vec![ArenaChange {
			mutation: Mutation::Counted,
			uuid,
			path,
			metadata,
			content_kind: ContentKind::Unknown,
		}]
	}

	/// Tell clients what changed, at whatever grain this writer was built for.
	///
	/// Hidden entries are indexed and not announced, which is what directory
	/// listings do with them by default.
	fn tell_clients(&self, changes: &[ArenaChange]) {
		match self.notify {
			Notify::Silent => {}
			Notify::Each => {
				for change in changes {
					self.emit_one(change);
				}
			}
			Notify::Batched => self.emit_batch(changes),
		}
	}

	fn emit_one(&self, change: &ArenaChange) {
		use crate::infra::event::ResourceMetadata;

		if matches!(change.mutation, Mutation::Unchanged) {
			return;
		}

		if matches!(change.mutation, Mutation::Deleted) {
			self.event_bus.emit(Event::ResourceDeleted {
				resource_type: "file".to_string(),
				resource_id: change.uuid,
			});
			return;
		}

		if change.is_hidden() {
			return;
		}

		let Some(file) = self.file_for(change) else {
			return;
		};
		let Ok(resource) = serde_json::to_value(&file) else {
			return;
		};

		self.event_bus.emit(Event::ResourceChanged {
			resource_type: "file".to_string(),
			resource,
			metadata: Some(ResourceMetadata {
				no_merge_fields: vec!["sd_path".to_string()],
				alternate_ids: vec![],
				affected_paths: self.sd_paths(change.affected()),
			}),
		});
	}

	fn emit_batch(&self, changes: &[ArenaChange]) {
		use crate::infra::event::ResourceMetadata;

		let visible: Vec<&ArenaChange> = changes
			.iter()
			.filter(|change| {
				!change.is_hidden()
					&& !matches!(change.mutation, Mutation::Deleted | Mutation::Unchanged)
			})
			.collect();
		if visible.is_empty() {
			return;
		}

		let files: Vec<crate::domain::file::File> = visible
			.iter()
			.filter_map(|change| self.file_for(change))
			.collect();
		if files.is_empty() {
			return;
		}

		let affected = visible
			.iter()
			.flat_map(|change| change.affected())
			.collect::<Vec<_>>();

		self.event_bus.emit(Event::ResourceChangedBatch {
			resource_type: "file".to_string(),
			resources: serde_json::to_value(&files).unwrap_or_default(),
			metadata: Some(ResourceMetadata {
				no_merge_fields: vec!["sd_path".to_string()],
				alternate_ids: vec![],
				affected_paths: self.sd_paths(affected),
			}),
		});
	}

	fn file_for(&self, change: &ArenaChange) -> Option<crate::domain::file::File> {
		use crate::domain::addressing::SdPath;
		use crate::domain::file::File;

		let metadata = change.metadata.as_ref()?;
		let sd_path = SdPath::Physical {
			device_slug: crate::device::get_current_device_slug(),
			path: change.path.clone(),
		};
		let mut file = File::from_ephemeral(change.uuid, metadata, sd_path);
		file.content_kind = change.content_kind;
		Some(file)
	}

	fn sd_paths(&self, paths: Vec<PathBuf>) -> Vec<crate::domain::addressing::SdPath> {
		use crate::domain::addressing::SdPath;

		let device_slug = crate::device::get_current_device_slug();
		let mut paths = paths;
		paths.sort();
		paths.dedup();
		paths
			.into_iter()
			.map(|path| SdPath::Physical {
				device_slug: device_slug.clone(),
				path,
			})
			.collect()
	}
}

#[async_trait::async_trait]
impl ChangeHandler for ArenaWriter {
	async fn find_by_path(&self, path: &Path) -> Result<Option<EntryRef>> {
		let index = self.index.read().await;
		let key = path.to_path_buf();

		Ok(index.get_entry_ref(&key).map(|metadata| EntryRef {
			id: 0,
			uuid: index.get_entry_uuid(&key),
			path: key,
			kind: metadata.kind,
		}))
	}

	async fn find_by_inode(&self, _inode: u64) -> Result<Option<EntryRef>> {
		// Inode tracking is skipped to minimize memory overhead; fall back to path-only detection.
		Ok(None)
	}

	async fn create(&mut self, metadata: &DirEntry, _parent_path: &Path) -> Result<EntryRef> {
		let entry = EntryMetadata::from(metadata.clone());
		let kind = entry.kind;
		let uuid = self
			.apply(Seen::Entries(vec![entry]))
			.await
			.first()
			.map(|change| change.uuid);

		Ok(EntryRef {
			id: 0,
			uuid,
			path: metadata.path.clone(),
			kind,
		})
	}

	async fn update(&mut self, _entry: &EntryRef, metadata: &DirEntry) -> Result<()> {
		self.apply(Seen::Entries(vec![EntryMetadata::from(metadata.clone())]))
			.await;
		Ok(())
	}

	async fn move_entry(
		&mut self,
		_entry: &EntryRef,
		old_path: &Path,
		new_path: &Path,
		_new_parent_path: &Path,
	) -> Result<()> {
		let metadata = build_dir_entry(new_path, None).await?;
		self.apply(Seen::Renamed {
			from: old_path.to_path_buf(),
			to: EntryMetadata::from(metadata),
		})
		.await;
		Ok(())
	}

	async fn delete(&mut self, entry: &EntryRef) -> Result<()> {
		self.apply(Seen::Lost {
			path: entry.path.clone(),
			is_directory: entry.is_directory(),
		})
		.await;
		Ok(())
	}

	async fn run_processors(&self, _entry: &EntryRef, _is_new: bool) -> Result<()> {
		// File processors (thumbnails, content hash) are disabled to ensure responsive, low-overhead browsing.
		Ok(())
	}

	async fn emit_change_event(&self, _entry: &EntryRef, _change_type: ChangeType) -> Result<()> {
		// Announcing is not a step this writer can be asked to take separately:
		// `apply` told everyone at the moment it wrote, from the same
		// description of the change. The trait keeps the hook for the database
		// handler, where the two really are separate writes.
		Ok(())
	}

	async fn handle_new_directory(&self, path: &Path) -> Result<()> {
		use crate::ops::indexing::metadata;

		let mut entries = match tokio::fs::read_dir(path).await {
			Ok(entries) => entries,
			Err(e) => {
				tracing::warn!(
					"Failed to read directory {} for ephemeral indexing: {}",
					path.display(),
					e
				);
				return Ok(());
			}
		};

		let mut observed = Vec::new();
		while let Ok(Some(entry)) = entries.next_entry().await {
			let entry_path = entry.path();
			let Ok(metadata) = entry.metadata().await else {
				continue;
			};
			let kind = if metadata.is_dir() {
				EntryKind::Directory
			} else if metadata.is_symlink() {
				EntryKind::Symlink
			} else {
				EntryKind::File
			};

			observed.push(EntryMetadata {
				path: entry_path.clone(),
				kind,
				size: metadata.len(),
				modified: metadata.modified().ok(),
				accessed: metadata.accessed().ok(),
				created: metadata.created().ok(),
				inode: crate::ops::indexing::metadata::get_inode(&entry_path, &metadata),
				permissions: None,
				is_hidden: is_hidden_path(&entry_path),
			});
		}

		self.apply(Seen::Entries(observed)).await;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::TempDir;

	fn file(path: &Path, size: u64) -> EntryMetadata {
		EntryMetadata {
			path: path.to_path_buf(),
			kind: EntryKind::File,
			size,
			modified: Some(std::time::SystemTime::now()),
			accessed: None,
			created: None,
			inode: None,
			permissions: None,
			is_hidden: is_hidden_path(path),
		}
	}

	fn writer(root: &Path, notify: Notify) -> (ArenaWriter, Arc<EventBus>) {
		let index = Arc::new(RwLock::new(EphemeralIndex::new().expect("arena")));
		let event_bus = Arc::new(EventBus::new(1024));
		let writer = ArenaWriter::new(index, event_bus.clone(), None).notifying(notify);
		(writer, event_bus)
	}

	async fn next_event(bus: &Arc<EventBus>) -> Option<Event> {
		let mut subscriber = bus.subscribe();
		tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv())
			.await
			.ok()
			.and_then(|event| event.ok())
	}

	#[tokio::test]
	async fn what_the_arena_takes_in_is_what_the_client_hears_about() {
		let root = TempDir::new().unwrap();
		let path = root.path().join("clip.mov");
		let (writer, bus) = writer(root.path(), Notify::Each);
		let mut subscriber = bus.subscribe();

		let changes = writer.apply(Seen::Entries(vec![file(&path, 12)])).await;

		assert_eq!(changes.len(), 1);
		assert_eq!(changes[0].mutation, Mutation::Created);
		assert!(writer.index.read().await.has_entry(&path));

		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv())
				.await
				.expect("an event")
				.expect("an event");

		let Event::ResourceChanged { resource, .. } = event else {
			panic!("expected ResourceChanged");
		};
		assert_eq!(
			resource["id"].as_str(),
			Some(changes[0].uuid.to_string().as_str()),
			"the event names the identity the arena kept"
		);
	}

	/// A second sighting of a path is a modification, and it keeps the identity
	/// the arena already holds rather than announcing a new one.
	#[tokio::test]
	async fn a_second_sighting_keeps_the_identity() {
		let root = TempDir::new().unwrap();
		let path = root.path().join("clip.mov");
		let (writer, _bus) = writer(root.path(), Notify::Each);

		let seen = file(&path, 12);
		let first = writer.apply(Seen::Entries(vec![seen.clone()])).await;
		let grown = EntryMetadata { size: 24, ..seen };
		let second = writer.apply(Seen::Entries(vec![grown])).await;

		assert_eq!(second[0].mutation, Mutation::Modified);
		assert_eq!(first[0].uuid, second[0].uuid);
		assert_eq!(first[0].content_kind, second[0].content_kind);
	}

	/// A walk over a drive that has been walked before finds almost nothing
	/// new. Announcing all of it would put every file nobody touched through
	/// serialisation on the way to a client that would render it identically.
	#[tokio::test]
	async fn a_re_walk_that_finds_nothing_new_says_nothing() {
		let root = TempDir::new().unwrap();
		let entries = vec![
			file(&root.path().join("one.txt"), 1),
			file(&root.path().join("two.txt"), 2),
		];
		let (writer, bus) = writer(root.path(), Notify::Batched);

		writer.apply(Seen::Entries(entries.clone())).await;
		let again = writer.apply(Seen::Entries(entries)).await;

		assert!(again
			.iter()
			.all(|change| change.mutation == Mutation::Unchanged));

		let mut subscriber = bus.subscribe();
		writer
			.apply(Seen::Entries(vec![file(&root.path().join("three.txt"), 3)]))
			.await;
		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv())
				.await
				.expect("the one new file is still announced")
				.expect("an event");
		let Event::ResourceChangedBatch { resources, .. } = event else {
			panic!("expected one batched event");
		};
		assert_eq!(resources.as_array().map(Vec::len), Some(1));
	}

	/// A walk of a folder someone opened: one event for the batch, and the
	/// hidden files in it are indexed without being announced.
	#[tokio::test]
	async fn a_walk_lands_as_one_event() {
		let root = TempDir::new().unwrap();
		let (writer, bus) = writer(root.path(), Notify::Batched);
		let mut subscriber = bus.subscribe();

		let entries = vec![
			file(&root.path().join("one.txt"), 1),
			file(&root.path().join("two.txt"), 2),
			file(&root.path().join(".hidden"), 3),
		];
		writer.apply(Seen::Entries(entries)).await;

		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv())
				.await
				.expect("an event")
				.expect("an event");

		let Event::ResourceChangedBatch { resources, .. } = event else {
			panic!("expected one batched event");
		};
		assert_eq!(
			resources.as_array().map(Vec::len),
			Some(2),
			"the hidden entry is indexed and not announced"
		);
		assert!(writer
			.index
			.read()
			.await
			.has_entry(&root.path().join(".hidden")));
	}

	/// Mapping a drive nobody asked to see: the arena fills and nothing is
	/// said, which is also why those entries can defer their identities.
	#[tokio::test]
	async fn a_silent_writer_says_nothing() {
		let root = TempDir::new().unwrap();
		let (writer, bus) = writer(root.path(), Notify::Silent);

		writer
			.apply(Seen::Entries(vec![file(&root.path().join("one.txt"), 1)]))
			.await;

		assert!(next_event(&bus).await.is_none());
		assert!(writer
			.index
			.read()
			.await
			.has_entry(&root.path().join("one.txt")));
	}

	/// A summary has no children to hear a change from, so recounting it is the
	/// only way its size stays true — and a size nobody is told about is a
	/// sunburst that quietly stops matching the drive.
	#[tokio::test]
	async fn a_recount_reaches_the_client() {
		let root = TempDir::new().unwrap();
		let dir = root.path().join("Library");
		std::fs::create_dir_all(&dir).unwrap();
		let (writer, bus) = writer(root.path(), Notify::Each);
		let mut subscriber = bus.subscribe();

		writer
			.apply(Seen::Entries(vec![EntryMetadata {
				kind: EntryKind::Directory,
				..file(&dir, 0)
			}]))
			.await;
		let _ =
			tokio::time::timeout(tokio::time::Duration::from_millis(50), subscriber.recv()).await;

		let changes = writer
			.apply(Seen::Counted {
				path: dir.clone(),
				totals: Rollup {
					bytes: 4_096,
					files: 9,
				},
			})
			.await;

		assert_eq!(changes.len(), 1);
		assert_eq!(changes[0].mutation, Mutation::Counted);
		assert_eq!(writer.index.read().await.subtree_size(&dir), Some(4_096));

		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv())
				.await
				.expect("a recount is a size change and has to be announced")
				.expect("an event");
		assert!(matches!(event, Event::ResourceChanged { .. }));
	}

	#[tokio::test]
	async fn a_lost_path_leaves_the_arena_and_the_client_is_told() {
		let root = TempDir::new().unwrap();
		let path = root.path().join("gone.txt");
		let (writer, bus) = writer(root.path(), Notify::Each);
		let mut subscriber = bus.subscribe();

		let created = writer.apply(Seen::Entries(vec![file(&path, 4)])).await;
		let _ =
			tokio::time::timeout(tokio::time::Duration::from_millis(50), subscriber.recv()).await;

		writer
			.apply(Seen::Lost {
				path: path.clone(),
				is_directory: false,
			})
			.await;

		assert!(!writer.index.read().await.has_entry(&path));

		let event =
			tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv())
				.await
				.expect("an event")
				.expect("an event");

		let Event::ResourceDeleted { resource_id, .. } = event else {
			panic!("expected ResourceDeleted");
		};
		assert_eq!(resource_id, created[0].uuid);
	}

	#[tokio::test]
	async fn an_entry_is_findable_by_path() {
		let root = TempDir::new().unwrap();
		let path = root.path().join("test.txt");
		let (mut writer, _bus) = writer(root.path(), Notify::Each);

		let dir_entry = DirEntry {
			path: path.clone(),
			kind: EntryKind::File,
			size: 12,
			modified: Some(std::time::SystemTime::now()),
			created: None,
			accessed: None,
			inode: Some(12345),
			permissions: None,
		};

		let created = writer
			.create(&dir_entry, root.path())
			.await
			.expect("create");
		assert!(created.uuid.is_some());

		let found = writer.find_by_path(&path).await.expect("find");
		assert_eq!(found.and_then(|entry| entry.uuid), created.uuid);
	}
}

/// A move is the change that has to name two places, and the one that has
/// already been written wrong once: the arena and the store were updated and
/// nothing was said, because emission was a separate step handed the path the
/// file had just left.
#[cfg(test)]
mod move_tests {
	use super::*;
	use tempfile::TempDir;

	async fn rearrange(from: &str, to: &str) -> Option<(serde_json::Value, Vec<String>)> {
		let root = TempDir::new().expect("root");
		let old = root.path().join(from);
		let new = root.path().join(to);
		for path in [&old, &new] {
			if let Some(parent) = path.parent() {
				std::fs::create_dir_all(parent).expect("parent");
			}
		}
		std::fs::write(&old, b"hello").expect("write");

		let index = Arc::new(RwLock::new(EphemeralIndex::new().expect("arena")));
		let event_bus = Arc::new(EventBus::new(1024));
		let mut subscriber = event_bus.subscribe();
		let mut writer = ArenaWriter::new(index, event_bus, None);

		let entry = DirEntry {
			path: old.clone(),
			kind: EntryKind::File,
			size: 5,
			modified: Some(std::time::SystemTime::now()),
			created: None,
			accessed: None,
			inode: Some(7),
			permissions: None,
		};
		let created = writer.create(&entry, root.path()).await.expect("create");
		// Drain the create event.
		let _ =
			tokio::time::timeout(tokio::time::Duration::from_millis(50), subscriber.recv()).await;

		std::fs::rename(&old, &new).expect("rename");
		writer
			.move_entry(&created, &old, &new, new.parent().unwrap())
			.await
			.expect("move");

		match tokio::time::timeout(tokio::time::Duration::from_millis(100), subscriber.recv()).await
		{
			Ok(Ok(Event::ResourceChanged {
				resource,
				metadata: Some(metadata),
				..
			})) => Some((
				resource,
				metadata
					.affected_paths
					.iter()
					.map(|path| path.to_string())
					.collect(),
			)),
			_ => None,
		}
	}

	#[tokio::test]
	async fn a_rename_reaches_the_ui() {
		let (resource, _) = rearrange("draft.txt", "final.txt")
			.await
			.expect("a rename the watcher saw must reach the client");

		assert!(
			resource["sd_path"].to_string().contains("final.txt"),
			"the event has to carry where the file is now, not where it was"
		);
	}

	#[tokio::test]
	async fn a_move_between_directories_reaches_the_ui() {
		let (resource, _) = rearrange("inbox/draft.txt", "archive/draft.txt")
			.await
			.expect("a move the watcher saw must reach the client");

		assert!(resource["sd_path"].to_string().contains("archive"));
	}

	/// A client renders directory listings, so a move invalidates two of them.
	/// An event naming only the destination leaves the file showing in the
	/// folder it left.
	#[tokio::test]
	async fn a_move_invalidates_both_listings() {
		let (_, affected) = rearrange("inbox/draft.txt", "archive/draft.txt")
			.await
			.expect("a move the watcher saw must reach the client");

		assert!(
			affected.iter().any(|path| path.contains("archive")),
			"the destination listing gains the file: {affected:?}"
		);
		assert!(
			affected.iter().any(|path| path.contains("inbox")),
			"the vacated listing loses it: {affected:?}"
		);
	}
}
