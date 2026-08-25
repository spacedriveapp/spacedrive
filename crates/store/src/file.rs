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

use std::collections::HashMap;
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
	pub inode: Option<i64>,
	pub mode: Option<i64>,
	pub extension: Option<String>,
	pub is_hidden: bool,
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
}

impl Ledger {
	/// Read every binding out of the store. One sequential scan, roughly sixty
	/// bytes a record held afterwards.
	pub async fn load(pool: &sqlx::SqlitePool) -> Result<Self> {
		let rows: Vec<(Uuid, String, i64, i64, Option<i64>)> = sqlx::query_as(
			"SELECT r.uuid, r.external_id, f.size, f.mtime, f.inode
			 FROM record r JOIN facet_file f ON f.record_uuid = r.uuid",
		)
		.fetch_all(pool)
		.await?;

		let mut ledger = Self::default();
		for (uuid, external_id, size, mtime, inode) in rows {
			ledger.bind(
				uuid,
				Binding {
					external_id: external_id.into(),
					size,
					mtime,
					inode,
				},
			);
		}
		Ok(ledger)
	}

	fn bind(&mut self, uuid: Uuid, binding: Binding) {
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

		Resolution::Fresh(Uuid::now_v7())
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

/// A batch's high-water mark, written in the same transaction as the batch.
pub struct Watermark<'a> {
	pub key: &'a str,
	pub value: &'a str,
}

const INSERT_FACET_FILE: &str = "\
INSERT INTO facet_file (record_uuid, size, mtime, inode, mode, extension, is_hidden)
 VALUES (?, ?, ?, ?, ?, ?, ?)
 ON CONFLICT (record_uuid) DO UPDATE SET
	size = excluded.size,
	mtime = excluded.mtime,
	inode = excluded.inode,
	mode = excluded.mode,
	extension = excluded.extension,
	is_hidden = excluded.is_hidden";

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
		watermark: Option<Watermark<'_>>,
	) -> Result<u64> {
		let mut tx = self.pool().begin().await?;
		let epoch = self.scan_epoch();
		let mut applied = 0;

		for write in writes {
			if !write.resolution.is_dirty() {
				continue;
			}

			let observation = &write.observation;
			let record = Record {
				uuid: write.uuid(),
				external_id: observation.external_id.clone(),
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

			sqlx::query(INSERT_FACET_FILE)
				.bind(record.uuid)
				.bind(observation.size)
				.bind(observation.mtime)
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
