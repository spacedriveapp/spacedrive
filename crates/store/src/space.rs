//! # Spaces
//!
//! A space is a folder inside a source that people and agents work in, plus
//! the policy that goes with it: whether its file contents are read, and later
//! which names an agent may not write and how long its change log is kept.
//! `docs/core/design/spaces.md` carries the model.
//!
//! A space is a person's choice, so it lives in the assertion half of the
//! store beside the tags, never in a generation table, and no rebuild touches
//! it. Rows merge the way tag definitions do: the later HLC stamp wins the
//! whole row, device uuid breaking ties. A deleted space keeps its row with
//! `deleted = 1`, so a store that was detached while the space was removed
//! cannot bring it back.

use uuid::Uuid;

use crate::db::{SourceDb, Stamp};
use crate::error::Result;

/// A space as its source store holds it.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SpaceRow {
	pub uuid: Uuid,
	pub name: String,
	/// `folder` for an existing folder, `workspace` for one Spacedrive
	/// created empty.
	pub kind: String,
	/// The folder, `/`-separated and relative to the source root. Empty is
	/// the whole source.
	pub root: String,
	/// Whether file contents inside the space are hashed and read.
	pub content_indexing: bool,
	/// Whether every change inside the space keeps its bytes as a version.
	pub versioning: bool,
	/// RFC 3339 time the space was created.
	pub created_at: String,
	pub deleted: bool,
	/// Sortable HLC text; the later stamp wins the whole row on merge.
	pub updated_hlc: String,
	pub origin_device: Uuid,
}

impl SpaceRow {
	/// The stamp this row was last written with.
	pub fn set_stamp(&mut self, stamp: &Stamp) {
		self.updated_hlc = stamp.hlc.clone();
		self.origin_device = stamp.device_uuid;
	}
}

const COLUMNS: &str = "uuid, name, kind, root, content_indexing, versioning, created_at, deleted, \
	updated_hlc, origin_device";

/// The same columns from a store written before versioning, whose rows all
/// read as versioning off.
const COLUMNS_BEFORE_VERSIONING: &str =
	"uuid, name, kind, root, content_indexing, 0 AS versioning, \
	created_at, deleted, updated_hlc, origin_device";

impl SourceDb {
	/// Every space this store holds that has not been deleted, by root.
	pub async fn spaces(&self) -> Result<Vec<SpaceRow>> {
		let read = |columns: &'static str| async move {
			let sql = format!("SELECT {columns} FROM space WHERE deleted = 0 ORDER BY root, name");
			sqlx::query_as(&sql).fetch_all(self.pool()).await
		};
		let rows = match read(COLUMNS).await {
			Err(error) if no_versioning_column(&error) => read(COLUMNS_BEFORE_VERSIONING).await,
			rows => rows,
		};
		Ok(older_store_has_none(rows)?.unwrap_or_default())
	}

	/// One space by id, deleted or not.
	pub async fn space(&self, uuid: Uuid) -> Result<Option<SpaceRow>> {
		let read = |columns: &'static str| async move {
			let sql = format!("SELECT {columns} FROM space WHERE uuid = ?");
			sqlx::query_as(&sql)
				.bind(uuid)
				.fetch_optional(self.pool())
				.await
		};
		let row = match read(COLUMNS).await {
			Err(error) if no_versioning_column(&error) => read(COLUMNS_BEFORE_VERSIONING).await,
			row => row,
		};
		Ok(older_store_has_none(row)?.flatten())
	}

	/// Write a space, unless the store already holds that space with a later
	/// stamp. Returns whether the row changed.
	pub async fn put_space(&self, row: &SpaceRow) -> Result<bool> {
		let result = sqlx::query(&format!(
			"INSERT INTO space ({COLUMNS}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
			 ON CONFLICT (uuid) DO UPDATE SET
				name = excluded.name,
				kind = excluded.kind,
				root = excluded.root,
				content_indexing = excluded.content_indexing,
				versioning = excluded.versioning,
				created_at = excluded.created_at,
				deleted = excluded.deleted,
				updated_hlc = excluded.updated_hlc,
				origin_device = excluded.origin_device
			 WHERE excluded.updated_hlc > space.updated_hlc
				OR (excluded.updated_hlc = space.updated_hlc
					AND excluded.origin_device > space.origin_device)"
		))
		.bind(row.uuid)
		.bind(&row.name)
		.bind(&row.kind)
		.bind(&row.root)
		.bind(row.content_indexing)
		.bind(row.versioning)
		.bind(&row.created_at)
		.bind(row.deleted)
		.bind(&row.updated_hlc)
		.bind(row.origin_device)
		.execute(self.pool())
		.await?;
		Ok(result.rows_affected() > 0)
	}
}

/// A replica copied from a store its owner has not migrated yet has the
/// space table without the versioning column.
fn no_versioning_column(error: &sqlx::Error) -> bool {
	matches!(error, sqlx::Error::Database(db) if db.message().contains("no such column: versioning"))
}

/// A store opened read-only runs no DDL, so one written before spaces, or a
/// replica copied from one, has no table: it holds no spaces.
fn older_store_has_none<T>(result: std::result::Result<T, sqlx::Error>) -> Result<Option<T>> {
	match result {
		Ok(value) => Ok(Some(value)),
		Err(sqlx::Error::Database(db)) if db.message().contains("no such table") => Ok(None),
		Err(error) => Err(error.into()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	async fn store() -> (tempfile::TempDir, SourceDb) {
		let dir = tempfile::tempdir().unwrap();
		let db = crate::SourceManager::new(dir.path().join("sources"))
			.ensure("drive-1", &crate::filesystem_schema())
			.await
			.unwrap();
		(dir, db)
	}

	fn row(hlc: &str, name: &str) -> SpaceRow {
		SpaceRow {
			uuid: Uuid::from_u128(1),
			name: name.into(),
			kind: "folder".into(),
			root: "Documents".into(),
			content_indexing: true,
			versioning: false,
			created_at: "2026-11-27T00:00:00Z".into(),
			deleted: false,
			updated_hlc: hlc.into(),
			origin_device: Uuid::from_u128(9),
		}
	}

	#[tokio::test]
	async fn the_later_stamp_wins_and_a_deleted_space_stays_deleted() {
		let (_dir, db) = store().await;
		assert!(db.put_space(&row("0002", "Docs")).await.unwrap());
		assert!(!db.put_space(&row("0001", "Older")).await.unwrap());
		assert_eq!(db.spaces().await.unwrap()[0].name, "Docs");

		let mut gone = row("0003", "Docs");
		gone.deleted = true;
		assert!(db.put_space(&gone).await.unwrap());
		assert!(!db.put_space(&row("0002", "Docs")).await.unwrap());
		assert!(db.spaces().await.unwrap().is_empty());
		assert!(db.space(gone.uuid).await.unwrap().unwrap().deleted);
	}

	#[tokio::test]
	async fn a_store_from_before_spaces_reads_as_having_none() {
		let (_dir, db) = store().await;
		sqlx::query("DROP TABLE space")
			.execute(db.pool())
			.await
			.unwrap();
		assert!(db.spaces().await.unwrap().is_empty());
		assert!(db.space(Uuid::from_u128(1)).await.unwrap().is_none());
	}
}
