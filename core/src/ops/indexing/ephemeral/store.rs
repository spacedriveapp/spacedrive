//! The durable half of a filesystem source.
//!
//! The arena answers reads and the store keeps identity; this is the seam
//! between them. [`MemoryAdapter`](super::MemoryAdapter) writes to both: the
//! arena inline, because a browse is waiting on it, and the store through a
//! channel, because SQLite is not on that path and must not become part of it.
//!
//! One writer task per source owns the [`Ledger`] and the [`SourceDb`]. Owning
//! them rather than sharing them is what keeps resolution lock-free — `resolve`
//! needs `&mut`, and a single consumer means nothing else can be halfway
//! through a rebind while it runs.
//!
//! The queue applies backpressure rather than dropping. A walk that outruns
//! SQLite waits; a store that quietly skipped observations would claim a
//! completeness it does not have, and the sweep would then read the gap as
//! deletion.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use sd_store::{
	filesystem_schema, FileKind, FileWrite, Ledger, Observation, SourceDb, SourceManager,
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::infra::source_dirs::SourceDirs;
use crate::ops::indexing::database_storage::EntryMetadata;
use crate::ops::indexing::state::EntryKind;

/// Resolved observations held before a batch commits. Large enough that a walk
/// commits in units of work rather than per file, small enough that losing the
/// uncommitted tail costs a fraction of a second of walking.
const BATCH_SIZE: usize = 512;

/// How long a partial batch waits for company before committing anyway. The
/// watcher delivers in ones and twos and should not sit in the queue.
const BATCH_LINGER: Duration = Duration::from_millis(250);

/// Depth of the queue between the arena and the writer.
const QUEUE_DEPTH: usize = 8192;

/// What the arena tells the store it saw.
enum Ingest {
	/// A path, with whatever the arena knew about it.
	Saw(Observation),
	/// A rename the watcher watched happen, so identity needs no guessing.
	Rename {
		from: String,
		observation: Observation,
	},
	/// A delete the watcher watched happen.
	Lost { external_id: String, subtree: bool },
	/// A complete walk of the whole source is starting; everything the store
	/// holds is presumed gone until this walk sees it.
	BeginSweep,
	/// That walk finished. What it never saw is deleted, except under the
	/// subtrees it could not read.
	FinishSweep { unreachable: Vec<String> },
	/// Commit what is pending and answer.
	Flush(oneshot::Sender<()>),
}

/// A filesystem source's durable store, and the task that writes it.
pub struct SourceStore {
	id: Uuid,
	root: PathBuf,
	db: Arc<SourceDb>,
	/// Dropping this ends the writer loop, which commits what it holds before
	/// it returns.
	tx: mpsc::Sender<Ingest>,
}

impl std::fmt::Debug for SourceStore {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SourceStore")
			.field("id", &self.id)
			.field("root", &self.root)
			.finish()
	}
}

impl SourceStore {
	/// Open a source's store, creating it on first attach, and start its
	/// writer with the ledger already loaded.
	pub async fn open(dirs: &SourceDirs, id: Uuid, root: PathBuf) -> Result<Arc<Self>> {
		let manager = SourceManager::new(dirs.root().to_path_buf());
		let db = manager
			.ensure(&id.simple().to_string(), &filesystem_schema())
			.await
			.with_context(|| format!("open source store for {id}"))?;
		let db = Arc::new(db);

		let ledger = Ledger::load(db.pool())
			.await
			.with_context(|| format!("load ledger for {id}"))?;
		tracing::debug!(source = %id, records = ledger.len(), "source store attached");

		let (tx, rx) = mpsc::channel(QUEUE_DEPTH);
		tokio::spawn(write_loop(db.clone(), ledger, rx));

		Ok(Arc::new(Self { id, root, db, tx }))
	}

	pub fn id(&self) -> Uuid {
		self.id
	}

	/// The store, for readers. Writers go through the queue.
	pub fn db(&self) -> &SourceDb {
		&self.db
	}

	/// Record what the arena just took in. Awaits when the queue is full.
	pub async fn saw(&self, metadata: &EntryMetadata) {
		let Some(observation) = self.observe(metadata) else {
			return;
		};
		self.send(Ingest::Saw(observation)).await;
	}

	/// Record a rename the watcher saw both ends of.
	pub async fn renamed(&self, from: &Path, metadata: &EntryMetadata) {
		let (Some(from), Some(observation)) = (self.external_id(from), self.observe(metadata))
		else {
			return;
		};
		self.send(Ingest::Rename { from, observation }).await;
	}

	/// Record a delete the watcher saw. A directory takes its subtree with it,
	/// which is one event on the way in and any number of records on the way
	/// out.
	pub async fn lost(&self, path: &Path, is_directory: bool) {
		let Some(external_id) = self.external_id(path) else {
			return;
		};
		self.send(Ingest::Lost {
			external_id,
			subtree: is_directory,
		})
		.await;
	}

	/// Open a sweep, so that what this walk does not see is deleted when it
	/// closes.
	///
	/// Only a complete, unfiltered enumeration of the whole source may open
	/// one. A walk that applied rules, stopped at a depth, or covered a single
	/// directory sees less than the source holds, and absence under it says
	/// nothing about what is on disk. `IndexerJobConfig::enumerates_whole_source`
	/// is that test.
	pub async fn begin_sweep(&self) {
		self.send(Ingest::BeginSweep).await;
	}

	/// Close a sweep, deleting what the walk never saw outside `unreachable`.
	///
	/// `unreachable` carries the directories the walk failed to open. A locked
	/// folder is a walk that did not look, so it costs its own subtree rather
	/// than the walk.
	pub async fn finish_sweep(&self, unreachable: &[PathBuf]) {
		let unreachable = unreachable
			.iter()
			.filter_map(|path| self.external_id(path))
			.collect();
		self.send(Ingest::FinishSweep { unreachable }).await;
	}

	/// Commit everything queued so far and wait for it to land.
	pub async fn flush(&self) {
		let (done, wait) = oneshot::channel();
		self.send(Ingest::Flush(done)).await;
		let _ = wait.await;
	}

	async fn send(&self, ingest: Ingest) {
		if self.tx.send(ingest).await.is_err() {
			tracing::warn!(source = %self.id, "source store writer is gone; observation dropped");
		}
	}

	/// A path's key in this source: relative to the root, so a drive that
	/// remounts elsewhere does not invalidate every row, and separated by `/`
	/// whatever wrote it, so a drive indexed on one platform reads on another.
	fn external_id(&self, path: &Path) -> Option<String> {
		let relative = path.strip_prefix(&self.root).ok()?;
		let key = relative.to_string_lossy();
		if key.is_empty() {
			return None;
		}
		Some(match std::path::MAIN_SEPARATOR {
			'/' => key.into_owned(),
			separator => key.replace(separator, "/"),
		})
	}

	fn observe(&self, metadata: &EntryMetadata) -> Option<Observation> {
		let external_id = self.external_id(&metadata.path)?;
		let name = metadata
			.path
			.file_name()
			.map(|n| n.to_string_lossy().into_owned())
			.unwrap_or_else(|| external_id.clone());

		Some(Observation {
			external_id,
			kind: match metadata.kind {
				EntryKind::File => FileKind::File,
				EntryKind::Directory => FileKind::Directory,
				EntryKind::Symlink => FileKind::Symlink,
			},
			name,
			size: metadata.size as i64,
			mtime: metadata.modified.map(unix_millis).unwrap_or(0),
			created: metadata.created.map(unix_millis),
			inode: metadata.inode.map(|i| i as i64),
			mode: metadata.permissions.map(|p| p as i64),
			extension: metadata
				.path
				.extension()
				.map(|e| e.to_string_lossy().into_owned()),
			is_hidden: metadata.is_hidden,
		})
	}
}

fn unix_millis(time: std::time::SystemTime) -> i64 {
	time.duration_since(UNIX_EPOCH)
		.map(|d| d.as_millis() as i64)
		.unwrap_or(0)
}

/// The containing directory's key, or `None` at the source root.
fn parent_of(external_id: &str) -> Option<&str> {
	external_id.rsplit_once('/').map(|(parent, _)| parent)
}

/// Resolve, batch and commit, until the last sender goes away.
async fn write_loop(db: Arc<SourceDb>, mut ledger: Ledger, mut rx: mpsc::Receiver<Ingest>) {
	let mut writes: Vec<FileWrite> = Vec::with_capacity(BATCH_SIZE);
	let mut removals: Vec<Uuid> = Vec::new();

	loop {
		// A partial batch waits out the linger; an empty one waits forever, so
		// an idle source costs nothing.
		let next = if writes.is_empty() && removals.is_empty() {
			rx.recv().await
		} else {
			match tokio::time::timeout(BATCH_LINGER, rx.recv()).await {
				Ok(next) => next,
				Err(_) => {
					commit(&db, &mut writes, &mut removals).await;
					continue;
				}
			}
		};

		let Some(ingest) = next else {
			commit(&db, &mut writes, &mut removals).await;
			return;
		};

		match ingest {
			Ingest::Saw(observation) => {
				let resolution = ledger.resolve(&observation);
				if resolution.is_dirty() {
					writes.push(FileWrite {
						resolution,
						parent_uuid: parent_of(&observation.external_id)
							.and_then(|parent| ledger.uuid_of(parent)),
						observation,
					});
				}
			}
			Ingest::Rename { from, observation } => {
				let resolution = ledger
					.rebind(&from, &observation)
					.unwrap_or_else(|| ledger.resolve(&observation));
				if resolution.is_dirty() {
					writes.push(FileWrite {
						resolution,
						parent_uuid: parent_of(&observation.external_id)
							.and_then(|parent| ledger.uuid_of(parent)),
						observation,
					});
				}
			}
			Ingest::Lost {
				external_id,
				subtree,
			} => {
				if subtree {
					removals.extend(ledger.forget_tree(&external_id));
				} else if let Some(uuid) = ledger.forget(&external_id) {
					removals.push(uuid);
				}
			}
			Ingest::BeginSweep => {
				// A sweep's verdict is "everything this walk did not see", so
				// anything still pending has to count as seen before it opens.
				commit(&db, &mut writes, &mut removals).await;
				ledger.begin_sweep();
			}
			Ingest::FinishSweep { unreachable } => {
				removals.extend(ledger.finish_sweep(&unreachable));
				commit(&db, &mut writes, &mut removals).await;
			}
			Ingest::Flush(done) => {
				commit(&db, &mut writes, &mut removals).await;
				let _ = done.send(());
			}
		}

		if writes.len() >= BATCH_SIZE {
			commit(&db, &mut writes, &mut removals).await;
		}
	}
}

/// Commit a batch, and drain it either way.
///
/// A failed batch is dropped rather than retried: the ledger has already bound
/// what it held, so replaying it would write rows the next walk resolves as
/// unchanged and never repairs. Losing it instead leaves the store behind the
/// filesystem, which is the state every walk is built to correct.
async fn commit(db: &SourceDb, writes: &mut Vec<FileWrite>, removals: &mut Vec<Uuid>) {
	if writes.is_empty() && removals.is_empty() {
		return;
	}

	match db.apply_files(writes, removals, None).await {
		Ok(applied) => tracing::trace!(applied, removed = removals.len(), "source store batch"),
		Err(error) => tracing::error!(%error, "source store batch failed"),
	}

	writes.clear();
	removals.clear();
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::event::EventBus;
	use crate::ops::indexing::change_detection::handler::ChangeHandler;
	use crate::ops::indexing::ephemeral::{EphemeralIndex, MemoryAdapter};
	use crate::ops::indexing::state::DirEntry;
	use tempfile::TempDir;
	use tokio::sync::RwLock;

	/// A source rooted in a temp directory, with the arena and the store both
	/// wired to a `MemoryAdapter` exactly as the watcher wires them.
	struct Fixture {
		_data: TempDir,
		root: TempDir,
		store: Arc<SourceStore>,
		adapter: MemoryAdapter,
	}

	impl Fixture {
		async fn new() -> Self {
			let data = TempDir::new().expect("data dir");
			let root = TempDir::new().expect("source root");
			let dirs = SourceDirs::new(data.path().join("sources")).expect("layout");

			let store = SourceStore::open(&dirs, Uuid::now_v7(), root.path().to_path_buf())
				.await
				.expect("store opens");

			let adapter = MemoryAdapter::new(
				Arc::new(RwLock::new(EphemeralIndex::new().expect("arena"))),
				Arc::new(EventBus::new(1024)),
				root.path().to_path_buf(),
				Some(store.clone()),
			);

			Self {
				_data: data,
				root,
				store,
				adapter,
			}
		}

		/// Create a real file and hand it to the adapter as the watcher would.
		async fn create(&mut self, relative: &str, contents: &[u8]) -> PathBuf {
			let path = self.root.path().join(relative);
			if let Some(parent) = path.parent() {
				std::fs::create_dir_all(parent).expect("parent");
			}
			std::fs::write(&path, contents).expect("write");
			self.adapter
				.create(&dir_entry(&path), self.root.path())
				.await
				.expect("create");
			path
		}

		async fn external_ids(&self) -> Vec<String> {
			self.store.flush().await;
			sqlx::query_scalar("SELECT external_id FROM record ORDER BY external_id")
				.fetch_all(self.store.db().pool())
				.await
				.expect("records")
		}
	}

	fn dir_entry(path: &Path) -> DirEntry {
		use std::hash::{Hash, Hasher};

		let metadata = std::fs::metadata(path).expect("metadata");
		// Distinct per path, and stable across a rename of the file behind it,
		// which is the only property the ledger reads an inode for.
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		metadata.len().hash(&mut hasher);
		path.file_name().hash(&mut hasher);

		DirEntry {
			path: path.to_path_buf(),
			kind: if metadata.is_dir() {
				EntryKind::Directory
			} else {
				EntryKind::File
			},
			size: metadata.len(),
			modified: metadata.modified().ok(),
			inode: Some(hasher.finish()),
		}
	}

	#[tokio::test]
	async fn what_the_arena_takes_in_reaches_the_store() {
		let mut fixture = Fixture::new().await;
		fixture.create("notes.txt", b"hello").await;
		fixture.create("photo.jpg", b"jpeg").await;

		assert_eq!(fixture.external_ids().await, vec!["notes.txt", "photo.jpg"]);
	}

	#[tokio::test]
	async fn keys_are_relative_to_the_source_root() {
		let mut fixture = Fixture::new().await;
		fixture.create("docs/deep/notes.txt", b"hello").await;

		// Absolute paths would tie every row to where the drive happened to
		// mount, which is the thing a source id exists to avoid.
		assert_eq!(fixture.external_ids().await, vec!["docs/deep/notes.txt"]);
	}

	#[tokio::test]
	async fn a_rename_keeps_one_record() {
		let mut fixture = Fixture::new().await;
		let old = fixture.create("draft.txt", b"hello").await;
		let uuid: Uuid = {
			fixture.store.flush().await;
			sqlx::query_scalar("SELECT uuid FROM record")
				.fetch_one(fixture.store.db().pool())
				.await
				.expect("record")
		};

		let new = fixture.root.path().join("final.txt");
		std::fs::rename(&old, &new).expect("rename");

		let entry = fixture
			.adapter
			.find_by_path(&old)
			.await
			.expect("lookup")
			.expect("known");
		fixture
			.adapter
			.move_entry(&entry, &old, &new, fixture.root.path())
			.await
			.expect("move");

		assert_eq!(fixture.external_ids().await, vec!["final.txt"]);

		let after: Uuid = sqlx::query_scalar("SELECT uuid FROM record")
			.fetch_one(fixture.store.db().pool())
			.await
			.expect("record");
		assert_eq!(after, uuid, "a rename is one row updated, not two rows");
	}

	#[tokio::test]
	async fn deleting_a_directory_takes_its_subtree() {
		let mut fixture = Fixture::new().await;
		let dir = fixture.root.path().join("docs");
		std::fs::create_dir_all(&dir).expect("dir");
		fixture
			.adapter
			.create(&dir_entry(&dir), fixture.root.path())
			.await
			.expect("create dir");
		fixture.create("docs/a.txt", b"a").await;
		fixture.create("docs/b.txt", b"b").await;
		fixture.create("keep.txt", b"keep").await;
		assert_eq!(fixture.external_ids().await.len(), 4);

		let parents: Vec<Option<Uuid>> =
			sqlx::query_scalar("SELECT parent_uuid FROM record WHERE external_id LIKE 'docs/%'")
				.fetch_all(fixture.store.db().pool())
				.await
				.expect("children");
		let directory: Uuid = sqlx::query_scalar("SELECT uuid FROM record WHERE external_id = ?")
			.bind("docs")
			.fetch_one(fixture.store.db().pool())
			.await
			.expect("directory");
		assert!(parents.iter().all(|parent| *parent == Some(directory)));

		let entry = fixture
			.adapter
			.find_by_path(&dir)
			.await
			.expect("lookup")
			.expect("known");
		fixture.adapter.delete(&entry).await.expect("delete");

		assert_eq!(fixture.external_ids().await, vec!["keep.txt"]);
	}

	#[tokio::test]
	async fn a_sweep_deletes_what_the_walk_did_not_see() {
		let mut fixture = Fixture::new().await;
		fixture.create("keep.txt", b"keep").await;
		let gone = fixture.create("gone.txt", b"gone").await;
		assert_eq!(fixture.external_ids().await.len(), 2);

		// The file leaves while nothing is watching, so the next walk learns
		// of it only by not finding it.
		std::fs::remove_file(&gone).expect("remove");
		fixture.store.begin_sweep().await;
		fixture.create("keep.txt", b"keep").await;
		fixture.store.finish_sweep(&[]).await;

		assert_eq!(fixture.external_ids().await, vec!["keep.txt"]);
	}

	#[tokio::test]
	async fn a_locked_folder_costs_its_subtree_not_the_walk() {
		let mut fixture = Fixture::new().await;
		fixture.create("open/a.txt", b"a").await;
		fixture.create("locked/b.txt", b"b").await;
		let gone = fixture.create("open/gone.txt", b"gone").await;
		assert_eq!(fixture.external_ids().await.len(), 3);

		// The walk cannot open `locked`, and reports nothing under it. That is
		// a walk that did not look, so its records stand; `open` was read, so
		// what is missing there really is gone.
		std::fs::remove_file(&gone).expect("remove");
		let locked = fixture.root.path().join("locked");
		fixture.store.begin_sweep().await;
		fixture.create("open/a.txt", b"a").await;
		fixture.store.finish_sweep(&[locked]).await;

		assert_eq!(
			fixture.external_ids().await,
			vec!["locked/b.txt", "open/a.txt"]
		);
	}

	#[tokio::test]
	async fn a_partition_with_no_store_still_browses() {
		let root = TempDir::new().expect("root");
		let path = root.path().join("notes.txt");
		std::fs::write(&path, b"hello").expect("write");

		let mut adapter = MemoryAdapter::new(
			Arc::new(RwLock::new(EphemeralIndex::new().expect("arena"))),
			Arc::new(EventBus::new(1024)),
			root.path().to_path_buf(),
			None,
		);

		adapter
			.create(&dir_entry(&path), root.path())
			.await
			.expect("scratch keeps working without anywhere durable to write");
		assert!(adapter.find_by_path(&path).await.expect("lookup").is_some());
	}
}
