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

		let rows: Vec<(Uuid, Option<Uuid>, Option<String>, i64, i64, Option<i64>)> =
			sqlx::query_as(
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
	/// | path miss, inode hit, size **or** mtime agrees | the file moved |
	/// | path miss, inode hit, both disagree | a new record |
	/// | path miss, inode miss | a new record |
	///
	/// Two factors are the minimum for a move, because an inode alone is not
	/// evidence: inode numbers are reused, and a wrong rebind silently moves
	/// one person's assertions onto an unrelated file. A missed rebind costs a
	/// re-tag, so the asymmetry decides the tie.
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
				binding.size == observation.size || binding.mtime == observation.mtime;
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

const INSERT_FACET_FILE: &str = "\
INSERT INTO facet_file (record_uuid, size, mtime, atime, inode, mode, extension, is_hidden)
 VALUES (?, ?, ?, ?, ?, ?, ?, ?)
 ON CONFLICT (record_uuid) DO UPDATE SET
	size = excluded.size,
	mtime = excluded.mtime,
	atime = excluded.atime,
	inode = excluded.inode,
	mode = excluded.mode,
	extension = excluded.extension,
	is_hidden = excluded.is_hidden";

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
pub fn filesystem_schema() -> DataTypeSchema {
	let mut fields = IndexMap::new();
	fields.insert("size".to_string(), FieldType::Integer);
	fields.insert("mtime".to_string(), FieldType::Integer);
	fields.insert("atime".to_string(), FieldType::Integer);
	fields.insert("inode".to_string(), FieldType::Integer);
	fields.insert("mode".to_string(), FieldType::Integer);
	fields.insert("extension".to_string(), FieldType::String);
	fields.insert("is_hidden".to_string(), FieldType::Boolean);

	let mut models = IndexMap::new();
	models.insert(
		"file".to_string(),
		ModelDef {
			fields,
			relations: RelationsDef::default(),
		},
	);

	DataTypeSchema {
		data_type: DataTypeMeta {
			id: "filesystem".to_string(),
			name: "Files".to_string(),
			icon: None,
		},
		models,
		search: SearchContract {
			primary_model: "file".to_string(),
			title: "name".to_string(),
			preview: "_derived.none".to_string(),
			subtitle: None,
			search_fields: Vec::new(),
			date_field: None,
		},
	}
}
