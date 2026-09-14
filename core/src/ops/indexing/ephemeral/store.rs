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
	filesystem_schema, ContentIdentity, FileKind, FileWrite, Ledger, Observation, SourceDb,
	SourceManager, SubtreeRename,
};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::infra::source_dirs::SourceDirs;
use crate::ops::indexing::metadata::EntryMetadata;
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
	/// The bytes behind some records have been identified.
	Identified(Vec<(Uuid, ContentIdentity)>),
	/// The bytes behind some records could not be read, and why.
	Unreadable(Vec<(Uuid, String)>),
	/// Commit what is pending and answer.
	Flush(oneshot::Sender<()>),
}

/// What a source's store holds, as the store counts it.
///
/// Directories are records like any other, so a count of files is `records`
/// less `directories`. `contents` is distinct sets of bytes, which is fewer
/// than `records` wherever the source holds the same file twice.
#[derive(Debug, Clone, Copy, Default)]
pub struct SourceCounts {
	pub records: u64,
	pub directories: u64,
	pub bytes: u64,
	pub contents: u64,
}

/// One copy of some bytes inside a source.
#[derive(Debug, Clone)]
pub struct DuplicateCopy {
	pub content_uuid: Uuid,
	pub size: u64,
	pub record_uuid: Uuid,
	pub path: PathBuf,
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

	/// Files this source holds whose bytes have not been identified yet.
	///
	/// Absolute paths, because the caller is going to open them. The store
	/// addresses everything relative to the root so that a drive remounting
	/// somewhere else does not invalidate the queue.
	pub async fn files_needing_content(&self, batch_size: usize) -> Vec<(Uuid, PathBuf, u64)> {
		match sd_store::files_needing_content(self.db.pool(), batch_size).await {
			Ok(pending) => pending
				.into_iter()
				.map(|file| {
					(
						file.uuid,
						self.root.join(&file.external_id),
						file.size.max(0) as u64,
					)
				})
				.collect(),
			Err(error) => {
				tracing::warn!(source = %self.id, %error, "could not list files needing content");
				Vec::new()
			}
		}
	}

	/// Files this source holds more than one copy of, largest first.
	///
	/// Absolute paths, since a caller is going to show them to someone or open
	/// them.
	pub async fn duplicates(&self, min_size: u64, group_limit: usize) -> Vec<DuplicateCopy> {
		match sd_store::duplicate_copies(self.db.pool(), min_size as i64, group_limit).await {
			Ok(copies) => copies
				.into_iter()
				.map(|copy| DuplicateCopy {
					content_uuid: copy.content_uuid,
					size: copy.size.unwrap_or(0).max(0) as u64,
					record_uuid: copy.record_uuid,
					path: self.root.join(&copy.external_id),
				})
				.collect(),
			Err(error) => {
				tracing::warn!(source = %self.id, %error, "could not list duplicates");
				Vec::new()
			}
		}
	}

	/// The identity of the bytes behind one of this source's records.
	pub async fn content_of(&self, record_uuid: Uuid) -> Option<Uuid> {
		sd_store::content_of(self.db.pool(), record_uuid)
			.await
			.unwrap_or_default()
	}

	/// Every copy of the given bytes that this source holds.
	///
	/// Absolute paths, for the same reason [`Self::duplicates`] gives them:
	/// a caller is going to show them to someone or open them.
	pub async fn copies_of_content(&self, content_uuid: Uuid) -> Vec<DuplicateCopy> {
		match sd_store::copies_of_content(self.db.pool(), content_uuid).await {
			Ok(copies) => copies
				.into_iter()
				.map(|copy| DuplicateCopy {
					content_uuid: copy.content_uuid,
					size: copy.size.unwrap_or(0).max(0) as u64,
					record_uuid: copy.record_uuid,
					path: self.root.join(&copy.external_id),
				})
				.collect(),
			Err(error) => {
				tracing::warn!(source = %self.id, %error, "could not list copies");
				Vec::new()
			}
		}
	}

	/// How many files are still waiting to be identified.
	pub async fn files_needing_content_count(&self) -> u64 {
		sd_store::count_files_needing_content(self.db.pool())
			.await
			.unwrap_or(0)
			.max(0) as u64
	}

	/// Record what the bytes behind these records turned out to be.
	pub async fn identified(&self, identities: Vec<(Uuid, ContentIdentity)>) {
		if identities.is_empty() {
			return;
		}
		self.send(Ingest::Identified(identities)).await;
	}

	/// Record why these files' bytes could not be read, so the pending set
	/// shrinks instead of handing them back on every pass.
	pub async fn content_unreadable(&self, failures: Vec<(Uuid, String)>) {
		if failures.is_empty() {
			return;
		}
		self.send(Ingest::Unreadable(failures)).await;
	}

	/// Write a dated, self-contained copy of this source's store.
	///
	/// `VACUUM INTO` runs against the live database without stopping the
	/// writer: the copy is a consistent snapshot as of its transaction, and
	/// everything queued behind it lands in the live store as usual. The
	/// flush first means the copy holds what the caller has already seen
	/// committed, not a state from before their last write.
	///
	/// This is what preserves a filesystem's indexed state before it gets
	/// reorganised: the live store follows the disk, and the next sweep
	/// forgets whatever a cleanup deleted. The frozen copy does not.
	pub async fn freeze_into(&self, dir: &Path) -> Result<PathBuf> {
		self.flush().await;

		std::fs::create_dir_all(dir)
			.with_context(|| format!("create freeze directory {}", dir.display()))?;
		let stamp = chrono::Utc::now().format("%Y-%m-%dT%H%M%SZ");
		let path = dir.join(format!("{stamp}.db"));
		if path.exists() {
			anyhow::bail!("a freeze already exists at {}", path.display());
		}

		sqlx::query("VACUUM INTO ?")
			.bind(path.to_str().with_context(|| {
				format!("freeze path is not valid UTF-8: {}", path.display())
			})?)
			.execute(self.db.pool())
			.await
			.with_context(|| format!("vacuum into {}", path.display()))?;

		tracing::info!(source = %self.id, path = %path.display(), "source frozen");
		Ok(path)
	}

	/// What this source actually persists: records, and the bytes behind them.
	///
	/// Read from the store rather than counted off the arena, because the arena
	/// maps the whole drive and a source is a scope over part of it. Asking the
	/// partition would report a nested source as owning everything around it.
	pub async fn counts(&self) -> Option<SourceCounts> {
		self.flush().await;
		let row: (i64, i64, Option<i64>, i64) = sqlx::query_as(
			"SELECT (SELECT COUNT(*) FROM record),
			        (SELECT COUNT(*) FROM record WHERE type = 'directory'),
			        (SELECT SUM(size) FROM facet_file),
			        (SELECT COUNT(*) FROM content)",
		)
		.fetch_one(self.db.pool())
		.await
		.ok()?;
		Some(SourceCounts {
			records: row.0.max(0) as u64,
			directories: row.1.max(0) as u64,
			bytes: row.2.unwrap_or(0).max(0) as u64,
			contents: row.3.max(0) as u64,
		})
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
			uid: metadata.uid.map(|u| u as i64),
			gid: metadata.gid.map(|g| g as i64),
			link_target: metadata.link_target.clone(),
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
			Ingest::Identified(identities) => {
				// Ordered behind whatever is staged: a record has to exist
				// before its content can point at it, and the batch that
				// created it may still be sitting here.
				commit(&db, &mut writes, &mut removals, &mut renames).await;
				if let Err(error) = db.set_content_identities(&identities).await {
					tracing::error!(%error, "content identities failed to land");
				}
			}
			Ingest::Unreadable(failures) => {
				commit(&db, &mut writes, &mut removals, &mut renames).await;
				if let Err(error) =
					sd_store::mark_content_unreadable(db.pool(), &failures).await
				{
					tracing::error!(%error, "content errors failed to land");
				}
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
	use crate::ops::indexing::ephemeral::{ArenaWriter, EphemeralIndex};
	use crate::ops::indexing::state::DirEntry;
	use tempfile::TempDir;
	use tokio::sync::RwLock;

	/// A source rooted in a temp directory, with the arena and the store both
	/// wired to an `ArenaWriter` exactly as the watcher wires them.
	struct Fixture {
		_data: TempDir,
		root: TempDir,
		store: Arc<SourceStore>,
		index: Arc<RwLock<EphemeralIndex>>,
		adapter: ArenaWriter,
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
			let adapter = ArenaWriter::new(
				index.clone(),
				Arc::new(EventBus::new(1024)),
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

		/// Rewrite a file the store already holds and re-observe it, the way
		/// a later walk would.
		async fn change(&mut self, relative: &str, contents: &[u8]) {
			let path = self.root.path().join(relative);
			std::fs::write(&path, contents).expect("rewrite");
			self.adapter
				.create(&dir_entry(&path), self.root.path())
				.await
				.expect("re-observe");
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
			uid: None,
			gid: None,
			link_target: None,
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

	/// A file is addressed by its parent and its name, so the queue of things
	/// left to hash has to rebuild the path from the join rather than read it
	/// out of a column. Storing it twice is what P2.6 removed.
	#[tokio::test]
	async fn the_hashing_queue_rebuilds_paths_from_parents() {
		let mut fixture = Fixture::new().await;
		let deep = fixture.create("photos/holiday/one.jpg", b"jpeg").await;
		let shallow = fixture.create("notes.txt", b"hello").await;
		fixture.store.flush().await;

		let mut pending = fixture.store.files_needing_content(10).await;
		pending.sort_by(|a, b| a.1.cmp(&b.1));

		// Directories have no bytes to identify, so only the two files are
		// waiting.
		assert_eq!(
			pending
				.iter()
				.map(|file| file.1.clone())
				.collect::<Vec<_>>(),
			vec![shallow, deep]
		);
		assert_eq!(fixture.store.files_needing_content_count().await, 2);
	}

	#[tokio::test]
	async fn an_identified_file_leaves_the_queue() {
		let mut fixture = Fixture::new().await;
		fixture.create("notes.txt", b"hello").await;
		fixture.store.flush().await;

		let pending = fixture.store.files_needing_content(10).await;
		assert_eq!(pending.len(), 1);

		fixture
			.store
			.identified(vec![(
				pending[0].0,
				ContentIdentity {
					sampled_hash: Some("0123456789abcdef".to_string()),
					integrity_hash: None,
					size: Some(5),
					kind: None,
				},
			)])
			.await;
		fixture.store.flush().await;

		assert!(fixture.store.files_needing_content(10).await.is_empty());
		assert_eq!(fixture.store.files_needing_content_count().await, 0);
	}

	/// An unreadable file leaves the queue with its reason recorded, so a
	/// permission-denied directory cannot fill every batch and starve the
	/// files behind it.
	#[tokio::test]
	async fn an_unreadable_file_leaves_the_queue_with_its_reason() {
		let mut fixture = Fixture::new().await;
		fixture.create("locked.bin", b"secret").await;
		fixture.create("open.bin", b"public").await;
		fixture.store.flush().await;

		let pending = fixture.store.files_needing_content(10).await;
		assert_eq!(pending.len(), 2);
		let locked = pending
			.iter()
			.find(|(_, path, _)| path.ends_with("locked.bin"))
			.expect("locked pending")
			.0;

		fixture
			.store
			.content_unreadable(vec![(locked, "permission denied".to_string())])
			.await;
		fixture.store.flush().await;

		let pending = fixture.store.files_needing_content(10).await;
		assert_eq!(pending.len(), 1, "the readable file is still waiting");
		assert!(pending[0].1.ends_with("open.bin"));
		assert_eq!(fixture.store.files_needing_content_count().await, 1);

		let error: Option<String> = sqlx::query_scalar(
			"SELECT content_error FROM facet_file f
			  JOIN record r ON r.uuid = f.record_uuid
			 WHERE r.title = 'locked.bin'",
		)
		.fetch_one(fixture.store.db().pool())
		.await
		.expect("content_error");
		assert_eq!(error.as_deref(), Some("permission denied"));
	}

	/// A changed file gets a fresh attempt: the facet rewrite clears the
	/// error the way it clears the content link.
	#[tokio::test]
	async fn a_changed_file_returns_to_the_queue_after_being_unreadable() {
		let mut fixture = Fixture::new().await;
		fixture.create("flaky.bin", b"v1").await;
		fixture.store.flush().await;

		let pending = fixture.store.files_needing_content(10).await;
		fixture
			.store
			.content_unreadable(vec![(pending[0].0, "io error".to_string())])
			.await;
		fixture.store.flush().await;
		assert_eq!(fixture.store.files_needing_content_count().await, 0);

		fixture.change("flaky.bin", b"v2 with more bytes").await;
		fixture.store.flush().await;

		assert_eq!(
			fixture.store.files_needing_content_count().await,
			1,
			"new bytes deserve a fresh attempt at reading them"
		);
	}

	/// The hashing job's loop against a real store: claim, identify, flush,
	/// repeat. The flush is what makes each claim see the last batch landed;
	/// without it the query outraces the writer and hands back the same
	/// files forever, which is a loop this test caught running against a
	/// real drive.
	#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
	async fn the_hashing_loop_terminates_against_a_live_writer() {
		let mut fixture = Fixture::new().await;
		for i in 0..700 {
			fixture
				.create(&format!("files/f{i:04}.bin"), format!("{i}").as_bytes())
				.await;
		}
		fixture.store.flush().await;

		let outstanding = fixture.store.files_needing_content_count().await;
		assert_eq!(outstanding, 700);

		let mut passes = 0;
		loop {
			let batch = fixture.store.files_needing_content(256).await;
			if batch.is_empty() {
				break;
			}
			passes += 1;
			assert!(
				passes < 100,
				"the loop is spinning: the pending set is not shrinking"
			);
			let identities = batch
				.iter()
				.map(|(uuid, _, size)| {
					(
						*uuid,
						ContentIdentity {
							sampled_hash: Some(format!("hash-{uuid}")),
							integrity_hash: None,
							size: Some(*size as i64),
							kind: None,
						},
					)
				})
				.collect();
			fixture.store.identified(identities).await;
			fixture.store.flush().await;
		}

		assert!(passes <= 4, "700 files at 256 a batch is three claims");
		assert_eq!(fixture.store.files_needing_content_count().await, 0);
	}

	/// A freeze survives what happens to the live store afterwards. That is
	/// its entire job: the live store mirrors the disk, and the frozen copy
	/// is the record of what the disk was.
	#[tokio::test]
	async fn a_freeze_keeps_what_the_live_store_later_forgets() {
		let mut fixture = Fixture::new().await;
		fixture.create("docs/before.txt", b"original state").await;
		fixture.create("keep.txt", b"stays").await;

		let freeze_dir = TempDir::new().expect("freeze dir");
		let frozen = fixture
			.store
			.freeze_into(freeze_dir.path())
			.await
			.expect("freeze");

		// The cleanup this feature exists to survive.
		let entry = fixture
			.adapter
			.find_by_path(&fixture.root.path().join("docs/before.txt"))
			.await
			.expect("lookup")
			.expect("known");
		fixture.adapter.delete(&entry).await.expect("delete");
		assert_eq!(fixture.paths().await, vec!["docs", "keep.txt"]);

		let pool = sqlx::sqlite::SqlitePoolOptions::new()
			.max_connections(1)
			.connect_with(
				sqlx::sqlite::SqliteConnectOptions::new()
					.filename(&frozen)
					.read_only(true)
					.immutable(true),
			)
			.await
			.expect("open frozen copy");
		let titles: Vec<String> =
			sqlx::query_scalar("SELECT title FROM record WHERE type = 'file' ORDER BY title")
				.fetch_all(&pool)
				.await
				.expect("frozen records");
		assert_eq!(
			titles,
			vec!["before.txt", "keep.txt"],
			"the deleted file is still in the freeze"
		);
	}

	/// Two copies of the same bytes are one row in `content`, pointed at by
	/// both records, and the duplicate query is what reads that back.
	#[tokio::test]
	async fn duplicates_are_grouped_by_bytes_not_by_name() {
		let mut fixture = Fixture::new().await;
		let one = fixture.create("photos/holiday.jpg", b"same").await;
		let two = fixture.create("backup/renamed.jpg", b"same").await;
		fixture.create("notes.txt", b"different").await;
		fixture.store.flush().await;

		let pending = fixture.store.files_needing_content(10).await;
		let identity = |hash: &str, size| ContentIdentity {
			sampled_hash: Some(hash.to_string()),
			integrity_hash: None,
			size: Some(size),
			kind: None,
		};
		let identified = pending
			.iter()
			.map(|(uuid, path, size)| {
				let hash = if path == &one || path == &two {
					"aaaaaaaaaaaaaaaa"
				} else {
					"bbbbbbbbbbbbbbbb"
				};
				(*uuid, identity(hash, *size as i64))
			})
			.collect();
		fixture.store.identified(identified).await;
		fixture.store.flush().await;

		let duplicates = fixture.store.duplicates(0, 10).await;
		let mut paths: Vec<_> = duplicates.iter().map(|copy| copy.path.clone()).collect();
		paths.sort();

		assert_eq!(paths, vec![two, one], "renaming a copy does not hide it");
		assert_eq!(
			duplicates[0].content_uuid, duplicates[1].content_uuid,
			"one identity for one set of bytes"
		);
	}

	/// Two copies of the same bytes are one row in `content`, pointed at by
	/// both records. That is the whole point of identifying them.
	#[tokio::test]
	async fn identical_bytes_share_one_content_row() {
		let mut fixture = Fixture::new().await;
		fixture.create("one.bin", b"same").await;
		fixture.create("two.bin", b"same").await;
		fixture.store.flush().await;

		let pending = fixture.store.files_needing_content(10).await;
		assert_eq!(pending.len(), 2);

		let identity = |uuid| {
			(
				uuid,
				ContentIdentity {
					sampled_hash: Some("deadbeefdeadbeef".to_string()),
					integrity_hash: None,
					size: Some(4),
					kind: None,
				},
			)
		};
		fixture
			.store
			.identified(vec![identity(pending[0].0), identity(pending[1].0)])
			.await;
		fixture.store.flush().await;

		let rows: (i64, i64) = sqlx::query_as(
			"SELECT (SELECT COUNT(*) FROM content),
					(SELECT COUNT(DISTINCT content_id) FROM record WHERE content_id IS NOT NULL)",
		)
		.fetch_one(fixture.store.db().pool())
		.await
		.expect("counts");

		assert_eq!(rows, (1, 1));
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

		let mut adapter = ArenaWriter::new(
			Arc::new(RwLock::new(EphemeralIndex::new().expect("arena"))),
			Arc::new(EventBus::new(1024)),
			None,
		);

		adapter
			.create(&dir_entry(&path), root.path())
			.await
			.expect("scratch keeps working without anywhere durable to write");
		assert!(adapter.find_by_path(&path).await.expect("lookup").is_some());
	}
}
