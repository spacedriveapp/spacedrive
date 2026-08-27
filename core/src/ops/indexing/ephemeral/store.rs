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
	SubtreeRename,
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
	/// Paths the walk or the watcher found. The reply carries the record uuid
	/// each one resolved to, which is the identity the arena then writes.
	Observe {
		observations: Vec<Observation>,
		identities: oneshot::Sender<Vec<Uuid>>,
	},
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

	/// Resolve what was seen to record identities, and take it in.
	///
	/// The ledger assigns identity, so this is where a file's uuid comes from:
	/// the arena writes what the store resolved rather than minting its own.
	/// Anything else gives one file two identities, and the tags on one of them
	/// never meet the assertions on the other.
	///
	/// Answers in input order. `None` for a path outside the source root, which
	/// the store has no key for.
	///
	/// Batched because a walk is: one round trip per batch rather than per
	/// file. Resolution itself is a hash lookup and the reply is sent before
	/// anything is committed, so the cost is the queue rather than a
	/// transaction. A commit already in flight does delay the next batch, which
	/// is the backpressure working rather than a stall.
	pub async fn identify(
		&self,
		metadata: &[EntryMetadata],
		known: &[Option<Uuid>],
	) -> Vec<Option<Uuid>> {
		let mut slots: Vec<Option<usize>> = Vec::with_capacity(metadata.len());
		let mut observations = Vec::with_capacity(metadata.len());

		for (index, entry) in metadata.iter().enumerate() {
			match self.observe(entry, known.get(index).copied().flatten()) {
				Some(observation) => {
					slots.push(Some(observations.len()));
					observations.push(observation);
				}
				None => slots.push(None),
			}
		}

		if observations.is_empty() {
			return vec![None; metadata.len()];
		}

		let (identities, resolved) = oneshot::channel();
		self.send(Ingest::Observe {
			observations,
			identities,
		})
		.await;

		let Ok(resolved) = resolved.await else {
			tracing::warn!(source = %self.id, "source store writer is gone; identities unresolved");
			return vec![None; metadata.len()];
		};

		slots
			.into_iter()
			.map(|slot| slot.and_then(|index| resolved.get(index).copied()))
			.collect()
	}

	/// Resolve one path's identity. The watcher's shape.
	pub async fn identify_one(
		&self,
		metadata: &EntryMetadata,
		known: Option<Uuid>,
	) -> Option<Uuid> {
		self.identify(std::slice::from_ref(metadata), &[known])
			.await
			.into_iter()
			.next()
			.flatten()
	}

	/// Record a rename the watcher saw both ends of.
	pub async fn renamed(&self, from: &Path, metadata: &EntryMetadata) {
		let (Some(from), Some(observation)) =
			(self.external_id(from), self.observe(metadata, None))
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

	/// What this source actually persists: records, and the bytes behind them.
	///
	/// Read from the store rather than counted off the arena, because the arena
	/// maps the whole drive and a source is a scope over part of it. Asking the
	/// partition would report a nested source as owning everything around it.
	pub async fn counts(&self) -> Option<(u64, u64)> {
		self.flush().await;
		let row: (i64, Option<i64>) = sqlx::query_as(
			"SELECT (SELECT COUNT(*) FROM record), (SELECT SUM(size) FROM facet_file)",
		)
		.fetch_one(self.db.pool())
		.await
		.ok()?;
		Some((row.0.max(0) as u64, row.1.unwrap_or(0).max(0) as u64))
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

	fn observe(&self, metadata: &EntryMetadata, known: Option<Uuid>) -> Option<Observation> {
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
			accessed: metadata.accessed.map(unix_millis),
			inode: metadata.inode.map(|i| i as i64),
			mode: metadata.permissions.map(|p| p as i64),
			extension: metadata
				.path
				.extension()
				.map(|e| e.to_string_lossy().into_owned()),
			is_hidden: metadata.is_hidden,
			identity: known,
		})
	}
}

/// The order to resolve a batch in: shallower paths first.
///
/// A record's parent is looked up through the ledger, so a directory has to be
/// bound before anything under it or the child is written with no parent and
/// the tree never forms. Discovery reads a directory before it reads what is
/// inside, but it reads many directories at once and a batch is filled from all
/// of them, so arrival order guarantees nothing. Depth does: a child is always
/// deeper than its parent, whatever order they were seen in.
fn shallowest_first(observations: &[Observation]) -> Vec<usize> {
	let mut order: Vec<usize> = (0..observations.len()).collect();
	order.sort_by_key(|&index| {
		observations[index]
			.external_id
			.bytes()
			.filter(|b| *b == b'/')
			.count()
	});
	order
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
	let mut renames: Vec<SubtreeRename> = Vec::new();

	loop {
		// A partial batch waits out the linger; an empty one waits forever, so
		// an idle source costs nothing.
		let next = if writes.is_empty() && removals.is_empty() && renames.is_empty() {
			rx.recv().await
		} else {
			match tokio::time::timeout(BATCH_LINGER, rx.recv()).await {
				Ok(next) => next,
				Err(_) => {
					commit(&db, &mut writes, &mut removals, &mut renames).await;
					continue;
				}
			}
		};

		let Some(ingest) = next else {
			commit(&db, &mut writes, &mut removals, &mut renames).await;
			return;
		};

		match ingest {
			Ingest::Observe {
				observations,
				identities,
			} => {
				let mut resolved: Vec<Option<Uuid>> = vec![None; observations.len()];

				for index in shallowest_first(&observations) {
					let observation = observations[index].clone();
					let resolution = ledger.resolve(&observation);
					resolved[index] = Some(resolution.uuid());
					if resolution.is_dirty() {
						writes.push(FileWrite {
							resolution,
							parent_uuid: parent_of(&observation.external_id)
								.and_then(|parent| ledger.uuid_of(parent)),
							observation,
						});
					}
				}

				let resolved: Vec<Uuid> = resolved.into_iter().flatten().collect();

				// A caller that stopped waiting is not an error: it already has
				// the arena's answer and the batch is staged either way.
				let _ = identities.send(resolved);
			}
			Ingest::Rename { from, observation } => {
				// A directory takes its subtree's addresses with it. Its own
				// record moves; everything under it keeps its parent and its
				// name and is simply reached a different way.
				if matches!(observation.kind, FileKind::Directory)
					&& from != observation.external_id
				{
					ledger.rename_tree(&from, &observation.external_id);
					renames.push(SubtreeRename {
						from: from.clone(),
						to: observation.external_id.clone(),
					});
				}

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
				commit(&db, &mut writes, &mut removals, &mut renames).await;
				ledger.begin_sweep();
			}
			Ingest::FinishSweep { unreachable } => {
				removals.extend(ledger.finish_sweep(&unreachable));
				commit(&db, &mut writes, &mut removals, &mut renames).await;
			}
			Ingest::Flush(done) => {
				commit(&db, &mut writes, &mut removals, &mut renames).await;
				let _ = done.send(());
			}
		}

		if writes.len() >= BATCH_SIZE {
			commit(&db, &mut writes, &mut removals, &mut renames).await;
		}
	}
}

/// Commit a batch, and drain it either way.
///
/// A failed batch is dropped rather than retried: the ledger has already bound
/// what it held, so replaying it would write rows the next walk resolves as
/// unchanged and never repairs. Losing it instead leaves the store behind the
/// filesystem, which is the state every walk is built to correct.
async fn commit(
	db: &SourceDb,
	writes: &mut Vec<FileWrite>,
	removals: &mut Vec<Uuid>,
	renames: &mut Vec<SubtreeRename>,
) {
	if writes.is_empty() && removals.is_empty() && renames.is_empty() {
		return;
	}

	match db.apply_files(writes, removals, renames, None).await {
		Ok(applied) => tracing::trace!(applied, removed = removals.len(), "source store batch"),
		Err(error) => tracing::error!(%error, "source store batch failed"),
	}

	writes.clear();
	removals.clear();
	renames.clear();
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
		index: Arc<RwLock<EphemeralIndex>>,
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

			let index = Arc::new(RwLock::new(EphemeralIndex::new().expect("arena")));
			let adapter = MemoryAdapter::new(
				index.clone(),
				Arc::new(EventBus::new(1024)),
				root.path().to_path_buf(),
				Some(store.clone()),
			);

			Self {
				_data: data,
				root,
				store,
				index,
				adapter,
			}
		}

		fn adapter_index(&self) -> &Arc<RwLock<EphemeralIndex>> {
			&self.index
		}

		/// Create a real file and hand it to the adapter as the watcher would,
		/// along with any directory above it that a walk would already have
		/// indexed. A file is addressed through its parent, so a parent that
		/// was never recorded leaves it reachable only by its bare name.
		async fn create(&mut self, relative: &str, contents: &[u8]) -> PathBuf {
			let path = self.root.path().join(relative);
			if let Some(parent) = path.parent() {
				std::fs::create_dir_all(parent).expect("parent");
			}

			let mut ancestors: Vec<PathBuf> = path
				.ancestors()
				.skip(1)
				.take_while(|a| *a != self.root.path())
				.map(|a| a.to_path_buf())
				.collect();
			ancestors.reverse();
			// Re-observed every time, as a walk does. A directory that is
			// already known resolves unchanged and writes nothing, and during a
			// sweep being observed is what keeps it from being condemned.
			for directory in ancestors {
				self.adapter
					.create(&dir_entry(&directory), self.root.path())
					.await
					.expect("create directory");
			}

			std::fs::write(&path, contents).expect("write");
			self.adapter
				.create(&dir_entry(&path), self.root.path())
				.await
				.expect("create");
			path
		}

		/// Every record's path. Only directories store one, so a file's is
		/// rebuilt from its parent's the way anything reading this store has to.
		async fn paths(&self) -> Vec<String> {
			self.store.flush().await;
			sqlx::query_scalar(&format!("SELECT {PATH_OF_RECORD} ORDER BY 1"))
				.fetch_all(self.store.db().pool())
				.await
				.expect("records")
		}

		async fn uuid_at(&self, path: &str) -> Option<Uuid> {
			self.store.flush().await;
			self.store.db().resolve_path(path).await.expect("query")
		}
	}

	const PATH_OF_RECORD: &str = "\
COALESCE(own.path, parent.path || '/' || r.title, r.title)
   FROM record r
   LEFT JOIN directory_path own ON own.record_uuid = r.uuid
   LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid";

	fn dir_entry(path: &Path) -> DirEntry {
		use std::hash::{Hash, Hasher};

		let metadata = std::fs::metadata(path).expect("metadata");
		// Distinct per path, and stable across a rename of the file behind it,
		// which is the only property the ledger reads an inode for.
		let mut hasher = std::collections::hash_map::DefaultHasher::new();
		metadata.len().hash(&mut hasher);
		path.file_name().hash(&mut hasher);

		#[cfg(unix)]
		let permissions = {
			use std::os::unix::fs::MetadataExt;
			Some(metadata.mode())
		};
		#[cfg(not(unix))]
		let permissions = None;

		DirEntry {
			path: path.to_path_buf(),
			kind: if metadata.is_dir() {
				EntryKind::Directory
			} else {
				EntryKind::File
			},
			size: metadata.len(),
			modified: metadata.modified().ok(),
			created: metadata.created().ok(),
			accessed: metadata.accessed().ok(),
			inode: Some(hasher.finish()),
			permissions,
		}
	}

	/// A source created over an already-mapped drive keeps the identities the
	/// map handed out.
	///
	/// The arena maps a whole drive whether or not anything is persisted off
	/// it, so by the time someone registers a source the files under it already
	/// have uuids. A store starting empty resolves every one of them as new,
	/// and minting there would leave one file with two identities: the one the
	/// UI has been using and the one the record table now claims.
	#[tokio::test]
	async fn a_source_over_a_mapped_drive_adopts_its_identities() {
		let fixture = Fixture::new().await;
		let root = fixture.root.path();
		std::fs::write(root.join("mapped.txt"), b"hello").expect("file");

		// The map got there first, as it does for any drive that was browsed
		// or indexed before anything was kept off it.
		let metadata = EntryMetadata::from(dir_entry(&root.join("mapped.txt")));
		let mapped = Uuid::now_v7();
		{
			let mut index = fixture.index.write().await;
			index
				.add_entry(root.join("mapped.txt"), mapped, metadata.clone())
				.expect("map it");
		}

		let identified = fixture
			.store
			.identify_one(&metadata, Some(mapped))
			.await
			.expect("identified");
		assert_eq!(identified, mapped, "the store minted a second identity");

		fixture.store.flush().await;
		let stored = fixture
			.store
			.db()
			.resolve_path("mapped.txt")
			.await
			.expect("query")
			.expect("record");
		assert_eq!(stored, mapped, "the record and the map name it differently");
	}

	/// A walk hands a batch over in whatever order discovery produced it, and
	/// the tree has to form anyway.
	///
	/// This is what a live index got wrong. 2,123,008 records walked and 1,349
	/// of them parented, because a child resolved before its own directory
	/// found nothing to link to and nothing ever looked again. The batch here
	/// is deliberately deepest-first, which is what consuming discovery's
	/// batches back to front produced.
	#[tokio::test]
	async fn a_batch_builds_the_tree_whatever_order_it_arrives_in() {
		let fixture = Fixture::new().await;
		let root = fixture.root.path();

		std::fs::create_dir_all(root.join("a/b/c")).expect("dirs");
		std::fs::write(root.join("a/b/c/deep.txt"), b"deep").expect("file");
		std::fs::write(root.join("a/b/sibling.txt"), b"sibling").expect("file");

		let relative = ["a/b/c/deep.txt", "a/b/sibling.txt", "a/b/c", "a/b", "a"];
		let metadata: Vec<EntryMetadata> = relative
			.iter()
			.map(|r| EntryMetadata::from(dir_entry(&root.join(r))))
			.collect();

		let identities = fixture.store.identify(&metadata, &[]).await;
		assert!(
			identities.iter().all(Option::is_some),
			"every observation resolves to a record"
		);
		fixture.store.flush().await;

		let db = fixture.store.db();
		let pool = db.pool();
		for (child, parent) in [
			("a/b", "a"),
			("a/b/c", "a/b"),
			("a/b/c/deep.txt", "a/b/c"),
			("a/b/sibling.txt", "a/b"),
		] {
			let child_uuid = db.resolve_path(child).await.expect("query").expect(child);
			let found: Option<Uuid> =
				sqlx::query_scalar("SELECT parent_uuid FROM record WHERE uuid = ?")
					.bind(child_uuid)
					.fetch_one(pool)
					.await
					.expect("query");
			let expected = db.resolve_path(parent).await.expect("query").expect(parent);
			assert_eq!(found, Some(expected), "{child} should sit under {parent}");
		}

		let top = db.resolve_path("a").await.expect("query").expect("a");
		let unparented: i64 = sqlx::query_scalar(
			"SELECT COUNT(*) FROM record WHERE parent_uuid IS NULL AND uuid <> ?",
		)
		.bind(top)
		.fetch_one(pool)
		.await
		.expect("count");
		assert_eq!(unparented, 0, "only the top level has no parent");
	}

	#[tokio::test]
	async fn what_the_arena_takes_in_reaches_the_store() {
		let mut fixture = Fixture::new().await;
		fixture.create("notes.txt", b"hello").await;
		fixture.create("photo.jpg", b"jpeg").await;

		assert_eq!(fixture.paths().await, vec!["notes.txt", "photo.jpg"]);
	}

	#[tokio::test]
	async fn keys_are_relative_to_the_source_root() {
		let mut fixture = Fixture::new().await;
		fixture.create("docs/deep/notes.txt", b"hello").await;

		// Absolute paths would tie every row to where the drive happened to
		// mount, which is the thing a source id exists to avoid.
		assert_eq!(
			fixture.paths().await,
			vec!["docs", "docs/deep", "docs/deep/notes.txt"]
		);
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

		assert_eq!(fixture.paths().await, vec!["final.txt"]);

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
		assert_eq!(fixture.paths().await.len(), 4);

		let parents: Vec<Option<Uuid>> = sqlx::query_scalar(&format!(
			"SELECT r.parent_uuid FROM record r
				   JOIN directory_path parent ON parent.record_uuid = r.parent_uuid
				  WHERE parent.path = 'docs'"
		))
		.fetch_all(fixture.store.db().pool())
		.await
		.expect("children");
		let directory = fixture.uuid_at("docs").await.expect("directory");
		assert!(parents.iter().all(|parent| *parent == Some(directory)));

		let entry = fixture
			.adapter
			.find_by_path(&dir)
			.await
			.expect("lookup")
			.expect("known");
		fixture.adapter.delete(&entry).await.expect("delete");

		assert_eq!(fixture.paths().await, vec!["keep.txt"]);
	}

	/// The whole point of the exercise. Tags hang off the arena's uuid and
	/// assertions hang off the record's; if those differ, nothing a person said
	/// about a file can ever be found from the file.
	#[tokio::test]
	async fn the_arena_and_the_record_name_a_file_the_same_way() {
		let mut fixture = Fixture::new().await;
		let path = fixture.create("notes.txt", b"hello").await;

		let in_arena = {
			let index = fixture.adapter_index().read().await;
			index
				.get_entry_uuid(&path)
				.expect("the arena identified it")
		};

		fixture.store.flush().await;
		let in_store: Uuid = sqlx::query_scalar(&format!(
			"SELECT r.uuid FROM record r
			   LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid
			  WHERE COALESCE(parent.path || '/' || r.title, r.title) = ?"
		))
		.bind("notes.txt")
		.fetch_one(fixture.store.db().pool())
		.await
		.expect("the store identified it");

		assert_eq!(in_arena, in_store);
	}

	/// A second sighting resolves to the record that already exists rather than
	/// minting beside it, which is what makes a re-walk free.
	#[tokio::test]
	async fn re_identifying_a_known_path_returns_the_same_record() {
		let fixture = Fixture::new().await;
		let path = fixture.root.path().join("notes.txt");
		std::fs::write(&path, b"hello").expect("write");
		let metadata = EntryMetadata::from(dir_entry(&path));

		let first = fixture
			.store
			.identify_one(&metadata, None)
			.await
			.expect("first");
		let again = fixture
			.store
			.identify_one(&metadata, None)
			.await
			.expect("again");

		assert_eq!(first, again);
	}

	/// A path outside the source has no key in this store, and answering with
	/// an invented uuid would bind it to a record that cannot exist.
	#[tokio::test]
	async fn a_path_outside_the_source_gets_no_identity() {
		let fixture = Fixture::new().await;
		let outside = TempDir::new().expect("elsewhere");
		let path = outside.path().join("stray.txt");
		std::fs::write(&path, b"hello").expect("write");

		let metadata = EntryMetadata::from(dir_entry(&path));
		assert!(fixture.store.identify_one(&metadata, None).await.is_none());
	}

	#[tokio::test]
	async fn a_sweep_deletes_what_the_walk_did_not_see() {
		let mut fixture = Fixture::new().await;
		fixture.create("keep.txt", b"keep").await;
		let gone = fixture.create("gone.txt", b"gone").await;
		assert_eq!(fixture.paths().await.len(), 2);

		// The file leaves while nothing is watching, so the next walk learns
		// of it only by not finding it.
		std::fs::remove_file(&gone).expect("remove");
		fixture.store.begin_sweep().await;
		fixture.create("keep.txt", b"keep").await;
		fixture.store.finish_sweep(&[]).await;

		assert_eq!(fixture.paths().await, vec!["keep.txt"]);
	}

	#[tokio::test]
	async fn a_locked_folder_costs_its_subtree_not_the_walk() {
		let mut fixture = Fixture::new().await;
		fixture.create("open/a.txt", b"a").await;
		fixture.create("locked/b.txt", b"b").await;
		let gone = fixture.create("open/gone.txt", b"gone").await;
		// Three files and the two directories they are addressed through.
		assert_eq!(fixture.paths().await.len(), 5);

		// The walk cannot open `locked`, and reports nothing under it. That is
		// a walk that did not look, so its records stand; `open` was read, so
		// what is missing there really is gone.
		std::fs::remove_file(&gone).expect("remove");
		let locked = fixture.root.path().join("locked");
		fixture.store.begin_sweep().await;
		fixture.create("open/a.txt", b"a").await;
		fixture.store.finish_sweep(&[locked]).await;

		assert_eq!(
			fixture.paths().await,
			vec!["locked", "locked/b.txt", "open", "open/a.txt"]
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
