//! The filesystem ingest: how a walk becomes rows in the same store an adapter
//! writes.
//!
//! One file format, two writers. A filesystem source gets the record table
//! every other source has, plus one facet table, and nothing about its shape is
//! special. What forks is ingest, because the two have nothing in common: an
//! adapter is a subprocess streaming JSON deltas resumed from a cursor, and a
//! walk is an in-process enumeration of ten million typed entries whose keys
//! are unstable on both ends.
//!
//! ## Identity without a ledger table
//!
//! The plan called for an identity ledger beside the records. Written out, it
//! turned out to be `record.external_id` plus three columns of `facet_file`, so
//! it is a *procedure* over the record table rather than a table of its own
//! ([`Ledger::resolve`]).
//!
//! It runs in memory. The arena is the primary read path and cannot wait on
//! SQLite, so the whole binding loads into a pair of maps at attach
//! ([`Ledger::load`]) and resolution stays a hash lookup. That is also why
//! `facet_file.inode` carries no index: the only reader is one sequential scan
//! at attach, and an index on it would cost a write on every file in the
//! source to serve a query nobody makes.
//!
//! The consequence, and the point of the exercise: record uuids stop living in
//! `ephemeral.snapshot`. The snapshot goes back to being a cache with nothing
//! durable in it, and all of its discard paths become harmless.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use indexmap::IndexMap;
use uuid::Uuid;

use crate::db::SourceDb;
use crate::error::Result;
use crate::record::{insert_record_query, Record};
use crate::schema::{
	DataTypeMeta, DataTypeSchema, FieldType, ModelDef, RelationsDef, SearchContract,
};

/// One `record JOIN facet_file` row: uuid, parent uuid, title, size, mtime, inode.
type LedgerRow = (Uuid, Option<Uuid>, Option<String>, i64, i64, Option<i64>);

/// One pending-content row: uuid, parent uuid, parent path, title, size.
type PendingContentRow = (Uuid, Option<Uuid>, Option<String>, Option<String>, i64);

/// A pending-content row plus the sampled hash to verify against.
type PendingVerificationRow = (
	Uuid,
	Option<Uuid>,
	Option<String>,
	Option<String>,
	i64,
	Option<String>,
);

/// One copy of a content row: content uuid, size, record uuid, parent uuid,
/// parent path, title.
type ContentCopyRow = (
	Uuid,
	Option<i64>,
	Uuid,
	Option<Uuid>,
	Option<String>,
	Option<String>,
);

/// What the walk found at a path. `record.type` carries this, so a directory
/// is queryable without joining a facet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
	File,
	Directory,
	Symlink,
}

impl FileKind {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::File => "file",
			Self::Directory => "directory",
			Self::Symlink => "symlink",
		}
	}

	/// The inverse of [`Self::as_str`], for rows read back from `record.type`.
	pub fn parse(value: &str) -> Option<Self> {
		match value {
			"file" => Some(Self::File),
			"directory" => Some(Self::Directory),
			"symlink" => Some(Self::Symlink),
			_ => None,
		}
	}
}

/// One thing the walk or the watcher saw, before it has an identity.
#[derive(Debug, Clone)]
pub struct Observation {
	/// Path relative to the source root. Relative, so a drive that remounts
	/// somewhere else does not invalidate every row.
	pub external_id: String,
	pub kind: FileKind,
	/// The file name, which becomes `record.title`.
	pub name: String,
	pub size: i64,
	/// Unix milliseconds.
	pub mtime: i64,
	pub created: Option<i64>,
	pub accessed: Option<i64>,
	pub inode: Option<i64>,
	pub mode: Option<i64>,
	pub uid: Option<i64>,
	pub gid: Option<i64>,
	/// Where a symlink points, verbatim from `readlink`. The link itself is
	/// the record; the target is the fact it carries.
	pub link_target: Option<String>,
	pub extension: Option<String>,
	pub is_hidden: bool,
	/// The identity this path already carries, where something already knows
	/// it. A ledger with no binding for a path adopts this rather than minting,
	/// which is what lets a source created over an already-mapped drive keep
	/// the uuids the map handed out. Minting there would give one file two
	/// identities and split everything hanging off them.
	pub identity: Option<Uuid>,
}

/// What [`Ledger::resolve`] made of an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
	/// Same path, same size and mtime. Nothing to write.
	Unchanged(Uuid),
	/// Same path, different bytes. The facet is stale and so is any content
	/// identity hanging off it.
	Changed(Uuid),
	/// The path is new and the inode is not: the file moved, and it keeps the
	/// identity everything a person said about it is attached to.
	Moved(Uuid),
	/// Nothing recognised it.
	Fresh(Uuid),
}

impl Resolution {
	pub fn uuid(&self) -> Uuid {
		match self {
			Self::Unchanged(u) | Self::Changed(u) | Self::Moved(u) | Self::Fresh(u) => *u,
		}
	}

	/// Whether this resolution has anything to write.
	pub fn is_dirty(&self) -> bool {
		!matches!(self, Self::Unchanged(_))
	}
}

/// Whether `external_id` is `root` or sits under it. External ids are
/// `/`-separated whatever wrote them, so this holds on every platform.
fn under(external_id: &str, root: &str) -> bool {
	external_id == root
		|| (external_id.len() > root.len()
			&& external_id.starts_with(root)
			&& external_id.as_bytes()[root.len()] == b'/')
}

/// A record's path: its own if it is a directory, otherwise its parent's plus
/// its name.
fn external_id_of(
	directories: &HashMap<Uuid, Arc<str>>,
	uuid: Uuid,
	parent_uuid: Option<Uuid>,
	title: Option<&str>,
) -> Option<Arc<str>> {
	if let Some(path) = directories.get(&uuid) {
		return Some(path.clone());
	}

	let title = title?;
	match parent_uuid {
		Some(parent) => {
			let parent_path = directories.get(&parent)?;
			Some(Arc::from(format!("{parent_path}/{title}").as_str()))
		}
		// Directly under the source root, which stores no path of its own.
		None => Some(Arc::from(title)),
	}
}

#[derive(Debug, Clone)]
struct Binding {
	external_id: Arc<str>,
	size: i64,
	mtime: i64,
	inode: Option<i64>,
}

/// Every record uuid in a filesystem source, indexed the two ways resolution
/// asks about them.
#[derive(Debug, Default)]
pub struct Ledger {
	bindings: HashMap<Uuid, Binding>,
	by_path: HashMap<Arc<str>, Uuid>,
	by_inode: HashMap<i64, Uuid>,
	/// Bindings a sweep has not seen yet, or `None` when no sweep is running.
	/// Held only for the duration of a walk, so the resting cost of a ledger
	/// is the bindings and the two indexes.
	unseen: Option<HashSet<Uuid>>,
}

impl Ledger {
	/// Read every binding out of the store. One sequential scan, roughly sixty
	/// bytes a record held afterwards.
	///
	/// Only directories store a path, so a file's is rebuilt here from its
	/// parent's. The ledger holds full paths in memory either way: resolution
	/// asks *what is bound at this path* on every observation, and a join per
	/// question is not a trade worth making for a table that is read once.
	pub async fn load(pool: &sqlx::SqlitePool) -> Result<Self> {
		let directories: HashMap<Uuid, Arc<str>> =
			sqlx::query_as::<_, (Uuid, String)>("SELECT record_uuid, path FROM directory_path")
				.fetch_all(pool)
				.await?
				.into_iter()
				.map(|(uuid, path)| (uuid, Arc::from(path.as_str())))
				.collect();

		let rows: Vec<LedgerRow> = sqlx::query_as(
			"SELECT r.uuid, r.parent_uuid, r.title, f.size, f.mtime, f.inode
			 FROM record r JOIN facet_file f ON f.record_uuid = r.uuid",
		)
		.fetch_all(pool)
		.await?;

		let mut ledger = Self::default();
		for (uuid, parent_uuid, title, size, mtime, inode) in rows {
			let Some(external_id) =
				external_id_of(&directories, uuid, parent_uuid, title.as_deref())
			else {
				// A record whose parent has no path is unaddressable, which is
				// the shape a half-written generation leaves behind. Leaving it
				// unbound means the next walk resolves it fresh rather than
				// binding an identity to a path that cannot be produced.
				continue;
			};
			ledger.bind(
				uuid,
				Binding {
					external_id,
					size,
					mtime,
					inode,
				},
			);
		}
		Ok(ledger)
	}

	fn bind(&mut self, uuid: Uuid, binding: Binding) {
		if let Some(unseen) = &mut self.unseen {
			unseen.remove(&uuid);
		}
		if let Some(previous) = self.bindings.get(&uuid) {
			self.by_path.remove(&previous.external_id);
			if let Some(inode) = previous.inode {
				self.by_inode.remove(&inode);
			}
		}
		self.by_path.insert(binding.external_id.clone(), uuid);
		if let Some(inode) = binding.inode {
			self.by_inode.insert(inode, uuid);
		}
		self.bindings.insert(uuid, binding);
	}

	/// The record at a path, if the source has seen it.
	pub fn uuid_of(&self, external_id: &str) -> Option<Uuid> {
		self.by_path.get(external_id).copied()
	}

	/// How many records are bound.
	pub fn len(&self) -> usize {
		self.bindings.len()
	}

	pub fn is_empty(&self) -> bool {
		self.bindings.is_empty()
	}

	/// Give an observation an identity, and record the binding it implies.
	///
	/// | evidence | outcome |
	/// |---|---|
	/// | path hit, size and mtime unchanged | the same record, nothing written |
	/// | path hit, either changed | the same record, facet stale |
	/// | path miss, inode hit, size **and** mtime agree | the file moved |
	/// | path miss, inode hit, either disagrees | a new record |
	/// | path miss, inode miss | a new record |
	///
	/// Three factors are the minimum for a move, because an inode alone is
	/// not evidence: inode numbers are reused, and a wrong rebind silently
	/// moves one person's assertions onto an unrelated file. A move also
	/// keeps the record's content row, whose integrity hash then stands for
	/// the bytes at the new path, so the bytes have to look untouched on both
	/// counts. A missed rebind costs a re-tag and a re-hash, so the asymmetry
	/// decides the tie.
	///
	/// A rename the watcher actually saw needs none of this, since it carries
	/// old and new together. Resolution is for the gap: the daemon was off, the
	/// drive was in a drawer, the walk was interrupted.
	pub fn resolve(&mut self, observation: &Observation) -> Resolution {
		let resolution = self.decide(observation);

		self.bind(
			resolution.uuid(),
			Binding {
				external_id: observation.external_id.as_str().into(),
				size: observation.size,
				mtime: observation.mtime,
				inode: observation.inode,
			},
		);

		resolution
	}

	fn decide(&self, observation: &Observation) -> Resolution {
		if let Some(uuid) = self.by_path.get(observation.external_id.as_str()) {
			let binding = &self.bindings[uuid];
			return if binding.size == observation.size && binding.mtime == observation.mtime {
				Resolution::Unchanged(*uuid)
			} else {
				Resolution::Changed(*uuid)
			};
		}

		if let Some(uuid) = observation.inode.and_then(|i| self.by_inode.get(&i)) {
			let binding = &self.bindings[uuid];
			let corroborated =
				binding.size == observation.size && binding.mtime == observation.mtime;
			if corroborated {
				return Resolution::Moved(*uuid);
			}
		}

		Resolution::Fresh(observation.identity.unwrap_or_else(Uuid::now_v7))
	}

	/// Rebind a record whose move the watcher actually saw.
	///
	/// A rename event carries both ends, so it needs none of [`Self::resolve`]'s
	/// evidence weighing: the kernel already said these are the same file.
	/// Returns `None` when the old path was never bound, leaving the caller to
	/// resolve the new path on its own evidence.
	pub fn rebind(
		&mut self,
		old_external_id: &str,
		observation: &Observation,
	) -> Option<Resolution> {
		let uuid = self.by_path.get(old_external_id).copied()?;
		self.bind(
			uuid,
			Binding {
				external_id: observation.external_id.as_str().into(),
				size: observation.size,
				mtime: observation.mtime,
				inode: observation.inode,
			},
		);
		Some(Resolution::Moved(uuid))
	}

	/// Start a sweep: every binding is presumed gone until an observation
	/// resolves against it.
	///
	/// A walk sees what is there and never sees what is not, so a deletion the
	/// daemon was not running for reaches the store only as an absence. The
	/// sweep is how absence becomes a removal. Starting a second sweep
	/// discards the first, which is what an interrupted walk should do: a
	/// partial enumeration is not evidence that the rest of the source is
	/// gone.
	pub fn begin_sweep(&mut self) {
		self.unseen = Some(self.bindings.keys().copied().collect());
	}

	/// End a sweep and forget everything it did not see, returning those
	/// records so the caller can remove them in the same batch.
	///
	/// `unreachable` names subtrees the walk could not enumerate — a folder it
	/// lacked permission to open, a mount that went away mid-walk. Absence
	/// under one of those is not evidence of deletion, only of a walk that did
	/// not look, so those bindings survive the sweep intact. One locked folder
	/// should cost its own subtree, not the walk.
	///
	/// Returns empty when no sweep is running, so a caller that never opened
	/// one cannot delete a source by accident. That also covers a walk resumed
	/// in a new process: the ledger reloaded without a sweep, and the resumed
	/// half of the enumeration is not grounds for condemning the first half.
	pub fn finish_sweep(&mut self, unreachable: &[String]) -> Vec<Uuid> {
		let Some(unseen) = self.unseen.take() else {
			return Vec::new();
		};

		// A sweep that wants to erase most of the store is evidence about the
		// walk, never about the files. A drive does not lose half its
		// contents between two scans; a walk loses its footing (unmounted
		// root, interrupted discovery, a resumed job with no memory of what
		// it saw). Refusing leaves stale rows for an honest walk to clean up
		// later, which is recoverable; obeying deleted 217k records once,
		// which was not.
		let total = self.bindings.len();
		if total > 100 && unseen.len() * 2 > total {
			tracing::error!(
				unseen = unseen.len(),
				total,
				"sweep refused: it saw too little of what the store holds"
			);
			return Vec::new();
		}

		let gone: Vec<Uuid> = unseen
			.into_iter()
			.filter(|uuid| {
				let Some(binding) = self.bindings.get(uuid) else {
					return false;
				};
				!unreachable
					.iter()
					.any(|prefix| under(&binding.external_id, prefix))
			})
			.collect();

		for uuid in &gone {
			if let Some(binding) = self.bindings.remove(uuid) {
				self.by_path.remove(&binding.external_id);
				if let Some(inode) = binding.inode {
					self.by_inode.remove(&inode);
				}
			}
		}
		gone
	}

	/// Whether a sweep is running.
	pub fn is_sweeping(&self) -> bool {
		self.unseen.is_some()
	}

	/// Forget a path and everything under it, which is what a directory delete
	/// comes to: one event in, a subtree out.
	///
	/// External ids are `/`-separated whatever wrote them, so the prefix test
	/// holds on every platform. It is a scan of the path index, which a walk
	/// would not tolerate and a delete does not care about.
	pub fn forget_tree(&mut self, external_id: &str) -> Vec<Uuid> {
		let gone: Vec<Uuid> = self
			.by_path
			.iter()
			.filter(|(path, _)| under(path, external_id))
			.map(|(_, uuid)| *uuid)
			.collect();

		for uuid in &gone {
			if let Some(binding) = self.bindings.remove(uuid) {
				self.by_path.remove(&binding.external_id);
				if let Some(inode) = binding.inode {
					self.by_inode.remove(&inode);
				}
			}
		}
		gone
	}

	/// Re-key a path and everything under it, which is what a directory rename
	/// comes to: one event in, a subtree re-addressed.
	///
	/// The records under a renamed directory do not change. Their paths are
	/// their parent's plus their name, and the parent is the only thing that
	/// moved. What has to change is this index, which holds full paths so that
	/// resolution can ask about one without a join.
	pub fn rename_tree(&mut self, from: &str, to: &str) {
		let moving: Vec<(Arc<str>, Uuid)> = self
			.by_path
			.iter()
			.filter(|(path, _)| under(path, from))
			.map(|(path, uuid)| (path.clone(), *uuid))
			.collect();

		for (path, uuid) in moving {
			let rekeyed: Arc<str> = Arc::from(format!("{to}{}", &path[from.len()..]).as_str());
			self.by_path.remove(&path);
			if let Some(binding) = self.bindings.get_mut(&uuid) {
				binding.external_id = rekeyed.clone();
			}
			self.by_path.insert(rekeyed, uuid);
		}
	}

	/// Forget a path. The watcher calls this on a delete it saw.
	pub fn forget(&mut self, external_id: &str) -> Option<Uuid> {
		let uuid = self.by_path.remove(external_id)?;
		if let Some(binding) = self.bindings.remove(&uuid) {
			if let Some(inode) = binding.inode {
				self.by_inode.remove(&inode);
			}
		}
		Some(uuid)
	}
}

/// A resolved observation, ready to write.
#[derive(Debug, Clone)]
pub struct FileWrite {
	pub resolution: Resolution,
	/// The containing directory's record, resolved through the ledger. `None`
	/// only at the source root.
	pub parent_uuid: Option<Uuid>,
	pub observation: Observation,
}

impl FileWrite {
	pub fn uuid(&self) -> Uuid {
		self.resolution.uuid()
	}
}

/// A directory that moved, and where it moved to. Both are source-relative.
///
/// Only directories store a path, so re-addressing a subtree is an update to
/// the directories inside it rather than to every record inside it.
#[derive(Debug, Clone)]
pub struct SubtreeRename {
	pub from: String,
	pub to: String,
}

/// A batch's high-water mark, written in the same transaction as the batch.
pub struct Watermark<'a> {
	pub key: &'a str,
	pub value: &'a str,
}

/// `content_error` resets on every rewrite: a facet row only changes when the
/// walk saw the file change, and new bytes deserve a fresh attempt at reading
/// them.
const INSERT_FACET_FILE: &str = "\
INSERT INTO facet_file (record_uuid, size, mtime, atime, inode, mode, uid, gid, link_target, extension, is_hidden)
 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
 ON CONFLICT (record_uuid) DO UPDATE SET
	size = excluded.size,
	mtime = excluded.mtime,
	atime = excluded.atime,
	inode = excluded.inode,
	mode = excluded.mode,
	uid = excluded.uid,
	gid = excluded.gid,
	link_target = excluded.link_target,
	extension = excluded.extension,
	is_hidden = excluded.is_hidden,
	content_error = NULL";

const INSERT_DIRECTORY_PATH: &str = "\
INSERT INTO directory_path (record_uuid, path) VALUES (?, ?)
 ON CONFLICT (record_uuid) DO UPDATE SET path = excluded.path";

/// The order a batch has to be written in, as indexes into it.
///
/// `record.parent_uuid` is a foreign key, so a child written ahead of a parent
/// arriving in the same transaction aborts the batch. Callers usually hand over
/// parents first and depending on that is a trap: the watcher reports what the
/// kernel coalesced, in the order it coalesced it.
fn parents_first(writes: &[FileWrite]) -> Vec<usize> {
	let position: HashMap<Uuid, usize> = writes
		.iter()
		.enumerate()
		.map(|(index, write)| (write.uuid(), index))
		.collect();

	let mut order = Vec::with_capacity(writes.len());
	let mut placed = vec![false; writes.len()];

	for start in 0..writes.len() {
		// Climb to the highest ancestor still inside this batch, then lay the
		// chain down from the top. `contains` closes the loop a parent cycle
		// would otherwise open; the filesystem cannot produce one, but a batch
		// is not the filesystem.
		let mut chain: Vec<usize> = Vec::new();
		let mut cursor = Some(start);
		while let Some(index) = cursor {
			if placed[index] || chain.contains(&index) {
				break;
			}
			chain.push(index);
			cursor = writes[index]
				.parent_uuid
				.and_then(|parent| position.get(&parent))
				.copied();
		}
		for index in chain.into_iter().rev() {
			placed[index] = true;
			order.push(index);
		}
	}

	order
}

impl SourceDb {
	/// Apply a batch of resolved observations, the removals the walk or the
	/// watcher observed, and the batch's watermark, in one transaction.
	///
	/// The point of the batch is that it is the unit of durability: kill the
	/// process partway through and the next run finds neither a half-written
	/// batch nor an advanced watermark, so it resumes from a state that
	/// actually happened.
	///
	/// Writes are keyed on the record uuid rather than the path, which is what
	/// makes a move an update to one row instead of a delete and an insert that
	/// would take the record's assertions with it.
	pub async fn apply_files(
		&self,
		writes: &[FileWrite],
		removals: &[Uuid],
		renames: &[SubtreeRename],
		watermark: Option<Watermark<'_>>,
	) -> Result<u64> {
		let mut tx = self.pool().begin().await?;
		let epoch = self.scan_epoch();
		let mut applied = 0;

		// Before the batch, so a directory written into the subtree by the same
		// batch lands at its new address rather than being moved twice.
		for rename in renames {
			sqlx::query(
				"UPDATE directory_path
				    SET path = ? || substr(path, ?)
				  WHERE path = ? OR (path >= ? AND path < ?)",
			)
			.bind(&rename.to)
			.bind(rename.from.len() as i64 + 1)
			.bind(&rename.from)
			.bind(format!("{}/", rename.from))
			.bind(format!("{}0", rename.from))
			.execute(&mut *tx)
			.await?;
		}

		for index in parents_first(writes) {
			let write = &writes[index];
			if !write.resolution.is_dirty() {
				continue;
			}

			let observation = &write.observation;
			// A directory carries its path; a file is found through its parent
			// and its name. See [`crate::record::RECORD_SCHEMA`].
			let is_directory = matches!(observation.kind, FileKind::Directory);
			let record = Record {
				uuid: write.uuid(),
				external_id: is_directory.then(|| observation.external_id.clone()),
				type_: observation.kind.as_str().to_string(),
				title: Some(observation.name.clone()),
				created_at: observation.created,
				modified_at: Some(observation.mtime),
				parent_uuid: write.parent_uuid,
				content_id: None,
			};

			insert_record_query(&record, epoch)
				.execute(&mut *tx)
				.await?;

			if is_directory {
				sqlx::query(INSERT_DIRECTORY_PATH)
					.bind(record.uuid)
					.bind(&observation.external_id)
					.execute(&mut *tx)
					.await?;
			}

			sqlx::query(INSERT_FACET_FILE)
				.bind(record.uuid)
				.bind(observation.size)
				.bind(observation.mtime)
				.bind(observation.accessed)
				.bind(observation.inode)
				.bind(observation.mode)
				.bind(observation.uid)
				.bind(observation.gid)
				.bind(&observation.link_target)
				.bind(&observation.extension)
				.bind(observation.is_hidden)
				.execute(&mut *tx)
				.await?;

			// The bytes moved under a content identity that described the old
			// ones. Dropping it re-queues the file for hashing rather than
			// leaving a hash that names something else.
			if matches!(write.resolution, Resolution::Changed(_)) {
				sqlx::query("UPDATE record SET content_id = NULL WHERE uuid = ?")
					.bind(record.uuid)
					.execute(&mut *tx)
					.await?;
			}

			applied += 1;
		}

		// Facet rows and edges cascade. Assertions do not, by design: the
		// row keeps its evidence and waits for a rebind.
		for uuid in removals {
			sqlx::query("DELETE FROM record WHERE uuid = ?")
				.bind(uuid)
				.execute(&mut *tx)
				.await?;
		}

		if let Some(watermark) = watermark {
			sqlx::query(
				"INSERT INTO _sync_state (key, value, updated_at)
				 VALUES (?, ?, datetime('now'))
				 ON CONFLICT (key) DO UPDATE SET
					value = excluded.value, updated_at = excluded.updated_at",
			)
			.bind(watermark.key)
			.bind(watermark.value)
			.execute(&mut *tx)
			.await?;
		}

		tx.commit().await?;
		Ok(applied)
	}
}

/// The filesystem data type.
///
/// Declared in Rust rather than as a TOML manifest, because nothing reads it
/// from disk and the walker writes typed columns directly. It is a real
/// [`DataTypeSchema`] all the same, so the facet DDL, the `_schema` row and the
/// migration diff all come from the same machinery every adapter uses.
///
/// The search contract is empty on purpose. The arena's name registry already
/// answers the prefix queries the UI issues and is faster at them than FTS5,
/// so an index here would cost a write per file to serve nothing. It earns its
/// place when full-text search over file *contents* arrives.
/// The files a source is still waiting to identify: everything with bytes and
/// no content row yet.
///
/// One clause, so the count and the batch can never disagree about what is
/// outstanding.
const PENDING_CONTENT: &str = "\
	FROM record r \
	JOIN facet_file f ON f.record_uuid = r.uuid \
	LEFT JOIN directory_path d ON d.record_uuid = r.parent_uuid \
	WHERE r.content_id IS NULL AND r.type = 'file' AND f.size > 0 \
	AND f.content_error IS NULL";

/// How many files are still waiting. What a progress bar needs, once.
pub async fn count_files_needing_content(pool: &sqlx::SqlitePool) -> Result<i64> {
	Ok(
		sqlx::query_scalar(&format!("SELECT COUNT(*) {PENDING_CONTENT}"))
			.fetch_one(pool)
			.await?,
	)
}

/// A file whose bytes have not been identified yet.
///
/// The path is relative to the source root, the way every address in the store
/// is: a drive that remounts somewhere else must not invalidate the work queue.
#[derive(Debug, Clone)]
pub struct PendingContent {
	pub uuid: Uuid,
	pub external_id: String,
	pub size: i64,
}

/// Files with no content identity, oldest record first.
///
/// A file is addressed through its parent and its name, so the path is rebuilt
/// by the same join [`Ledger::load`] uses rather than read from a column. That
/// is the point of storing it once.
///
/// Directories and symlinks have no bytes to identify. Neither does an empty
/// file: hashing nothing produces a hash every empty file on the machine would
/// share, which is a duplicate group nobody wants.
pub async fn files_needing_content(
	pool: &sqlx::SqlitePool,
	batch_size: usize,
) -> Result<Vec<PendingContent>> {
	let rows: Vec<PendingContentRow> = sqlx::query_as(&format!(
		"SELECT r.uuid, r.parent_uuid, d.path, r.title, f.size {PENDING_CONTENT} ORDER BY r.rowid LIMIT ?"
	))
	.bind(batch_size as i64)
	.fetch_all(pool)
	.await?;

	Ok(rows
		.into_iter()
		.filter_map(|(uuid, parent_uuid, parent_path, title, size)| {
			Some(PendingContent {
				uuid,
				external_id: address(parent_uuid, parent_path, title)?,
				size,
			})
		})
		.collect())
}

/// A shared-content file whose bytes have not been read in full.
///
/// The sampled tier says two files are probably the same; acting on that
/// (keeping one copy of two) needs the integrity tier, and only for the
/// files whose content is actually shared. A file on a candidate row is
/// claimed when any other record, confirmed or not, carries the same
/// sampled hash, so the second copy is still read after the first one
/// confirms. One clause, same as [`PENDING_CONTENT`], so the count and the
/// batch cannot disagree.
const PENDING_VERIFICATION: &str = "\
	FROM record r \
	JOIN content c ON c.id = r.content_id \
	JOIN facet_file f ON f.record_uuid = r.uuid \
	LEFT JOIN directory_path d ON d.record_uuid = r.parent_uuid \
	WHERE c.integrity_hash IS NULL AND f.content_error IS NULL \
	AND c.sampled_hash IN (SELECT c2.sampled_hash FROM record r2 \
		JOIN content c2 ON c2.id = r2.content_id \
		WHERE c2.sampled_hash IS NOT NULL GROUP BY c2.sampled_hash HAVING COUNT(*) > 1)";

/// A copy whose bytes want reading in full: where it is, and the sampled
/// hash its candidate row is keyed by, so the confirmed row the verdict
/// lands on keeps it and candidate lookups still reach it.
#[derive(Debug, Clone)]
pub struct PendingVerification {
	pub uuid: Uuid,
	pub external_id: String,
	pub size: i64,
	pub sampled_hash: Option<String>,
}

/// How many shared-content copies still carry only a sampled hash.
pub async fn count_files_needing_verification(pool: &sqlx::SqlitePool) -> Result<i64> {
	Ok(
		sqlx::query_scalar(&format!("SELECT COUNT(*) {PENDING_VERIFICATION}"))
			.fetch_one(pool)
			.await?,
	)
}

/// Shared-content copies whose bytes have not been read in full, in record
/// order. Largest groups are not privileged: the queue drains entirely, and
/// a caller that wants the big wins first can sort what it claims.
pub async fn files_needing_verification(
	pool: &sqlx::SqlitePool,
	batch_size: usize,
) -> Result<Vec<PendingVerification>> {
	let rows: Vec<PendingVerificationRow> = sqlx::query_as(&format!(
		"SELECT r.uuid, r.parent_uuid, d.path, r.title, f.size, c.sampled_hash \
			 {PENDING_VERIFICATION} ORDER BY r.rowid LIMIT ?"
	))
	.bind(batch_size as i64)
	.fetch_all(pool)
	.await?;

	Ok(rows
		.into_iter()
		.filter_map(
			|(uuid, parent_uuid, parent_path, title, size, sampled_hash)| {
				Some(PendingVerification {
					uuid,
					external_id: address(parent_uuid, parent_path, title)?,
					size,
					sampled_hash,
				})
			},
		)
		.collect())
}

/// Record why files could not be read, taking them out of the pending set.
///
/// Without this, an unreadable file stays `content_id IS NULL` forever and
/// every batch hands it back, so one permission-denied directory eventually
/// fills a whole batch and starves everything behind it. The error is kept
/// rather than a flag because the store is the record of what was and was not
/// captured, and "no identity" without a why is a gap that cannot be audited.
pub async fn mark_content_unreadable(
	pool: &sqlx::SqlitePool,
	failures: &[(Uuid, String)],
) -> Result<()> {
	let mut tx = pool.begin().await?;
	for (uuid, error) in failures {
		sqlx::query("UPDATE facet_file SET content_error = ? WHERE record_uuid = ?")
			.bind(error)
			.bind(uuid)
			.execute(&mut *tx)
			.await?;
	}
	tx.commit().await?;
	Ok(())
}

/// One copy of some bytes: a record, and where it is.
#[derive(Debug, Clone)]
pub struct ContentCopy {
	/// The identity of the bytes, derived from their hash, so two sources
	/// holding the same file report the same uuid without ever comparing notes.
	pub content_uuid: Uuid,
	pub size: Option<i64>,
	pub record_uuid: Uuid,
	/// Path relative to the source root.
	pub external_id: String,
}

type DuplicateRow = (
	String,
	Uuid,
	Option<String>,
	Option<i64>,
	Uuid,
	Option<Uuid>,
	Option<String>,
	Option<String>,
);

/// Files this source holds more than one copy of, largest first.
///
/// Grouped by sampled hash rather than by name or size, so a file renamed on
/// the way to its second home is still the same bytes, and a copy that has
/// been read in full still pairs with one that has not. Where members carry
/// different integrity hashes the group splits: each confirmed row makes a
/// group with the still-unread copies, and two files proven to hold
/// different bytes are never listed together. The `content_uuid` reported
/// is the confirmed uuid for a group anchored on a confirmed row and the
/// candidate uuid otherwise. The store reads the `group_limit` largest
/// sampled-hash groups, so a sampled hash whose members all split apart
/// counts against the limit without producing a group; the caller asked
/// for at most that many and gets at most that many. What this cannot see
/// is the copy that exists
/// once here and once on another drive: each store only knows its own, and
/// finding those means an index of content uuids across all of them.
pub async fn duplicate_copies(
	pool: &sqlx::SqlitePool,
	min_size: i64,
	group_limit: usize,
) -> Result<Vec<ContentCopy>> {
	let rows: Vec<DuplicateRow> = sqlx::query_as(
		"WITH duplicated AS (
				SELECT c.sampled_hash AS sampled_hash, MAX(c.size) AS size
				FROM record r
				JOIN content c ON c.id = r.content_id
				WHERE c.sampled_hash IS NOT NULL AND COALESCE(c.size, 0) >= ?
				GROUP BY c.sampled_hash
				HAVING COUNT(*) > 1
				ORDER BY size DESC
				LIMIT ?
			)
			SELECT c.sampled_hash, c.uuid, c.integrity_hash, c.size,
			       r.uuid, r.parent_uuid, d.path, r.title
			FROM duplicated
			JOIN content c ON c.sampled_hash = duplicated.sampled_hash
			JOIN record r ON r.content_id = c.id
			LEFT JOIN directory_path d ON d.record_uuid = r.parent_uuid
			ORDER BY duplicated.size DESC, c.sampled_hash, r.title",
	)
	.bind(min_size)
	.bind(group_limit as i64)
	.fetch_all(pool)
	.await?;

	struct Member {
		content_uuid: Uuid,
		integrity_hash: Option<String>,
		copy: ContentCopy,
	}
	let mut by_sample: Vec<(String, Vec<Member>)> = Vec::new();
	for (
		sampled,
		content_uuid,
		integrity_hash,
		size,
		record_uuid,
		parent_uuid,
		parent_path,
		title,
	) in rows
	{
		let Some(external_id) = address(parent_uuid, parent_path, title) else {
			continue;
		};
		let member = Member {
			content_uuid,
			integrity_hash,
			copy: ContentCopy {
				content_uuid,
				size,
				record_uuid,
				external_id,
			},
		};
		match by_sample.last_mut() {
			Some((last, members)) if *last == sampled => members.push(member),
			_ => by_sample.push((sampled, vec![member])),
		}
	}

	let mut copies = Vec::new();
	let mut groups = 0;
	for (_, members) in by_sample {
		if groups >= group_limit {
			break;
		}
		let (confirmed, candidates): (Vec<Member>, Vec<Member>) = members
			.into_iter()
			.partition(|member| member.integrity_hash.is_some());
		let mut anchors: Vec<(Uuid, Vec<Member>)> = Vec::new();
		for member in confirmed {
			match anchors
				.iter_mut()
				.find(|(uuid, _)| *uuid == member.content_uuid)
			{
				Some((_, group)) => group.push(member),
				None => anchors.push((member.content_uuid, vec![member])),
			}
		}
		if anchors.is_empty() {
			if candidates.len() > 1 {
				groups += 1;
				copies.extend(candidates.into_iter().map(|member| member.copy));
			}
			continue;
		}
		for (uuid, group) in anchors {
			if group.len() + candidates.len() < 2 {
				continue;
			}
			groups += 1;
			copies.extend(group.into_iter().map(|member| member.copy));
			copies.extend(candidates.iter().map(|member| ContentCopy {
				content_uuid: uuid,
				..member.copy.clone()
			}));
		}
	}
	Ok(copies)
}

/// The identity of the bytes behind a record, if they have been identified.
///
/// None is ordinary rather than exceptional: a record exists from the moment
/// the walk sees it, and its content row arrives later when the hash job gets
/// to it.
pub async fn content_of(pool: &sqlx::SqlitePool, record_uuid: Uuid) -> Result<Option<Uuid>> {
	Ok(sqlx::query_scalar(
		"SELECT c.uuid FROM record r JOIN content c ON c.id = r.content_id WHERE r.uuid = ?",
	)
	.bind(record_uuid)
	.fetch_optional(pool)
	.await?)
}

/// Every record in this source holding the given bytes.
///
/// Keyed by content rather than by record, so a caller can ask each source the
/// same question and get every copy on the machine. A copy under a different
/// name in a different directory is still found, because the identity is the
/// bytes. A candidate uuid reaches confirmed rows too, so a reference minted
/// before verification keeps finding every copy afterwards.
pub async fn copies_of_content(
	db: &crate::db::SourceDb,
	content_uuid: Uuid,
) -> Result<Vec<ContentCopy>> {
	let rows: Vec<ContentCopyRow> = sqlx::query_as(&format!(
		"SELECT c.uuid, c.size, r.uuid, r.parent_uuid, d.path, r.title
			 FROM content c
			 JOIN record r ON r.content_id = c.id
			 LEFT JOIN directory_path d ON d.record_uuid = r.parent_uuid
			 WHERE c.uuid = ? OR {} = ?
			 ORDER BY r.title",
		candidate_column(db.schema_version())
	))
	.bind(content_uuid)
	.bind(content_uuid)
	.fetch_all(db.pool())
	.await?;

	Ok(rows
		.into_iter()
		.filter_map(
			|(content_uuid, size, record_uuid, parent_uuid, parent_path, title)| {
				Some(ContentCopy {
					content_uuid,
					size,
					record_uuid,
					external_id: address(parent_uuid, parent_path, title)?,
				})
			},
		)
		.collect())
}

/// The column a candidate content uuid is matched against, for the shape a
/// handle reads. A store below schema version 1 has no `candidate_uuid`, and
/// a replica keeps its owner's version, so a reader of one matches `uuid`
/// twice rather than naming a column the table does not have.
pub(crate) fn candidate_column(schema_version: i64) -> &'static str {
	if schema_version >= 1 {
		"c.candidate_uuid"
	} else {
		"c.uuid"
	}
}

/// A record's path, rebuilt from its parent and its name.
///
/// Only directories store a path. A file is addressed through the one above it,
/// so this join is how any query that needs a path gets one, and why there is no
/// column to read.
fn address(
	parent_uuid: Option<Uuid>,
	parent_path: Option<String>,
	title: Option<String>,
) -> Option<String> {
	let title = title?;
	match (parent_uuid, parent_path) {
		(Some(_), Some(parent)) => Some(format!("{parent}/{title}")),
		// A record whose parent has no path is unaddressable, which is the
		// shape a half-written generation leaves behind. The next walk
		// re-parents it; until then there is nothing to open.
		(Some(_), None) => None,
		// Directly under the source root, which stores no path of its own.
		(None, _) => Some(title),
	}
}

/// One facet model from a flat field list, so a declaration reads as the
/// columns it produces rather than as builder calls.
fn model(fields: &[(&str, FieldType)]) -> ModelDef {
	ModelDef {
		fields: fields
			.iter()
			.map(|(name, ty)| ((*name).to_string(), *ty))
			.collect::<IndexMap<_, _>>(),
		relations: RelationsDef::default(),
	}
}

/// The image facet. Shape follows what an EXIF read yields: geometry, capture
/// time and place, the camera and lens that took it, and the rendering hints a
/// viewer needs before it has decoded anything.
fn image_model() -> ModelDef {
	model(&[
		("width", FieldType::Integer),
		("height", FieldType::Integer),
		("blurhash", FieldType::String),
		("date_taken", FieldType::Datetime),
		("latitude", FieldType::Float),
		("longitude", FieldType::Float),
		("camera_make", FieldType::String),
		("camera_model", FieldType::String),
		("lens_model", FieldType::String),
		("focal_length", FieldType::String),
		("aperture", FieldType::String),
		("shutter_speed", FieldType::String),
		("iso", FieldType::Integer),
		("orientation", FieldType::Integer),
		("color_space", FieldType::String),
		("color_profile", FieldType::String),
		("bit_depth", FieldType::String),
		("artist", FieldType::String),
		("copyright", FieldType::String),
		("description", FieldType::Text),
	])
}

/// The video facet. Frame rate is kept as a numerator and denominator because
/// 24000/1001 is not 23.976 and the difference accumulates over a timeline.
fn video_model() -> ModelDef {
	model(&[
		("width", FieldType::Integer),
		("height", FieldType::Integer),
		("blurhash", FieldType::String),
		("duration_seconds", FieldType::Float),
		("bit_rate", FieldType::Integer),
		("codec", FieldType::String),
		("pixel_format", FieldType::String),
		("color_space", FieldType::String),
		("color_range", FieldType::String),
		("color_primaries", FieldType::String),
		("color_transfer", FieldType::String),
		("fps_num", FieldType::Integer),
		("fps_den", FieldType::Integer),
		("audio_codec", FieldType::String),
		("audio_channels", FieldType::String),
		("audio_sample_rate", FieldType::Integer),
		("audio_bit_rate", FieldType::Integer),
		("title", FieldType::String),
		("artist", FieldType::String),
		("album", FieldType::String),
		("creation_time", FieldType::Datetime),
		("date_captured", FieldType::Datetime),
	])
}

/// The audio facet, carrying the tags a library sorts and groups by alongside
/// the stream properties.
fn audio_model() -> ModelDef {
	model(&[
		("duration_seconds", FieldType::Float),
		("bit_rate", FieldType::Integer),
		("sample_rate", FieldType::Integer),
		("channels", FieldType::String),
		("codec", FieldType::String),
		("title", FieldType::String),
		("artist", FieldType::String),
		("album", FieldType::String),
		("album_artist", FieldType::String),
		("genre", FieldType::String),
		("year", FieldType::Integer),
		("track_number", FieldType::Integer),
		("disc_number", FieldType::Integer),
		("composer", FieldType::String),
		("publisher", FieldType::String),
		("copyright", FieldType::String),
	])
}

pub fn filesystem_schema() -> DataTypeSchema {
	let mut models = IndexMap::new();
	models.insert(
		"file".to_string(),
		model(&[
			("size", FieldType::Integer),
			("mtime", FieldType::Integer),
			("atime", FieldType::Integer),
			("inode", FieldType::Integer),
			("mode", FieldType::Integer),
			("uid", FieldType::Integer),
			("gid", FieldType::Integer),
			("link_target", FieldType::String),
			("extension", FieldType::String),
			("is_hidden", FieldType::Boolean),
			// Why the last attempt to read this file's bytes failed, cleared
			// whenever the walk sees the file change. NULL means unattempted
			// or identified; `record.content_id` says which.
			("content_error", FieldType::String),
		]),
	);
	models.insert("image".to_string(), image_model());
	models.insert("video".to_string(), video_model());
	models.insert("audio".to_string(), audio_model());

	DataTypeSchema {
		data_type: DataTypeMeta {
			id: "filesystem".to_string(),
			name: "Files".to_string(),
			icon: None,
		},
		models,
		search: SearchContract {
			primary_model: "file".to_string(),
			// The display name is the filename, which the walk writes to
			// `record.title` rather than to any facet column.
			title: "_derived.title".to_string(),
			preview: "_derived.none".to_string(),
			subtitle: None,
			search_fields: Vec::new(),
			date_field: None,
		},
	}
}
