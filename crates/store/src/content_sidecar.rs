//! # Content sidecars
//!
//! `content_sidecar` lists what has been derived from the bytes a source holds:
//! proxies, speech audio, transcripts, shot lists, keyframes and the documents
//! extensions write. A row is keyed by the candidate content uuid, the uuid of
//! the sampled hash, because that one exists as soon as a file is sampled and
//! does not change when an integrity hash lands.
//!
//! The table is generation data. The files sit in the source's `derived/`
//! directory on the machine that made them, and a scan of that directory
//! rebuilds every row, so nothing here is reserved for sync.
//!
//! A kind name is only unique within whoever declared it: the core's kinds
//! are closed and an extension names its own. The producer is therefore part
//! of the key, beside the content uuid, kind, variant and format that address
//! a file.

use sqlx::Row;
use uuid::Uuid;

use crate::db::SourceDb;
use crate::error::Result;

/// The table, created on every writable open like the rest of the generation.
pub const CONTENT_SIDECAR_SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS content_sidecar (
    content_uuid BLOB NOT NULL,
    producer TEXT NOT NULL,
    kind TEXT NOT NULL,
    variant TEXT NOT NULL,
    format TEXT NOT NULL,
    producer_version TEXT,
    size INTEGER NOT NULL,
    checksum TEXT,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (content_uuid, producer, kind, variant, format)
) WITHOUT ROWID";

/// One derived artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentSidecarRow {
	pub content_uuid: Uuid,
	/// `core`, or the id of the extension that wrote it.
	pub producer: String,
	pub kind: String,
	pub variant: String,
	/// The file extension.
	pub format: String,
	pub producer_version: Option<String>,
	pub size: u64,
	/// BLAKE3 of the file, hex.
	pub checksum: Option<String>,
	/// Unix milliseconds.
	pub created_at: i64,
}

/// What identifies a row: everything but the facts about the file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentSidecarKey {
	pub content_uuid: Uuid,
	pub producer: String,
	pub kind: String,
	pub variant: String,
	pub format: String,
}

impl ContentSidecarRow {
	pub fn key(&self) -> ContentSidecarKey {
		ContentSidecarKey {
			content_uuid: self.content_uuid,
			producer: self.producer.clone(),
			kind: self.kind.clone(),
			variant: self.variant.clone(),
			format: self.format.clone(),
		}
	}
}

const COLUMNS: &str =
	"content_uuid, producer, kind, variant, format, producer_version, size, checksum, created_at";

fn row_of(row: sqlx::sqlite::SqliteRow) -> Option<ContentSidecarRow> {
	Some(ContentSidecarRow {
		content_uuid: row.try_get("content_uuid").ok()?,
		producer: row.try_get("producer").ok()?,
		kind: row.try_get("kind").ok()?,
		variant: row.try_get("variant").ok()?,
		format: row.try_get("format").ok()?,
		producer_version: row.try_get("producer_version").ok()?,
		size: row.try_get::<i64, _>("size").ok()?.max(0) as u64,
		checksum: row.try_get("checksum").ok()?,
		created_at: row.try_get("created_at").ok()?,
	})
}

impl SourceDb {
	/// Record an artifact, replacing what an earlier run wrote under its key.
	pub async fn put_content_sidecar(&self, row: &ContentSidecarRow) -> Result<()> {
		sqlx::query(&format!(
			"INSERT OR REPLACE INTO content_sidecar ({COLUMNS}) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
		))
		.bind(row.content_uuid)
		.bind(&row.producer)
		.bind(&row.kind)
		.bind(&row.variant)
		.bind(&row.format)
		.bind(&row.producer_version)
		.bind(row.size as i64)
		.bind(&row.checksum)
		.bind(row.created_at)
		.execute(self.pool())
		.await?;
		Ok(())
	}

	/// Forget one artifact. Answers whether a row was there.
	pub async fn delete_content_sidecar(&self, key: &ContentSidecarKey) -> Result<bool> {
		let done = sqlx::query(
			"DELETE FROM content_sidecar
				 WHERE content_uuid = ? AND producer = ? AND kind = ? AND variant = ? AND format = ?",
		)
		.bind(key.content_uuid)
		.bind(&key.producer)
		.bind(&key.kind)
		.bind(&key.variant)
		.bind(&key.format)
		.execute(self.pool())
		.await?;
		Ok(done.rows_affected() > 0)
	}

	/// Forget every artifact of one content.
	pub async fn delete_content_sidecars_of(&self, content_uuid: Uuid) -> Result<u64> {
		let done = sqlx::query("DELETE FROM content_sidecar WHERE content_uuid = ?")
			.bind(content_uuid)
			.execute(self.pool())
			.await?;
		Ok(done.rows_affected())
	}

	/// Every artifact of one content, ordered by producer, kind, variant and
	/// format. A store written before the table existed answers none.
	pub async fn content_sidecars(&self, content_uuid: Uuid) -> Result<Vec<ContentSidecarRow>> {
		if !self.has_content_sidecar_table().await? {
			return Ok(Vec::new());
		}
		let rows = sqlx::query(&format!(
			"SELECT {COLUMNS} FROM content_sidecar WHERE content_uuid = ?
				 ORDER BY producer, kind, variant, format"
		))
		.bind(content_uuid)
		.fetch_all(self.pool())
		.await?;
		Ok(rows.into_iter().filter_map(row_of).collect())
	}

	/// Every artifact this store lists.
	pub async fn all_content_sidecars(&self) -> Result<Vec<ContentSidecarRow>> {
		if !self.has_content_sidecar_table().await? {
			return Ok(Vec::new());
		}
		let rows = sqlx::query(&format!("SELECT {COLUMNS} FROM content_sidecar"))
			.fetch_all(self.pool())
			.await?;
		Ok(rows.into_iter().filter_map(row_of).collect())
	}

	/// How many artifacts this store lists and their total size.
	pub async fn content_sidecar_totals(&self) -> Result<(u64, u64)> {
		if !self.has_content_sidecar_table().await? {
			return Ok((0, 0));
		}
		let (count, size): (i64, i64) =
			sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(size), 0) FROM content_sidecar")
				.fetch_one(self.pool())
				.await?;
		Ok((count.max(0) as u64, size.max(0) as u64))
	}

	/// Content uuids with artifacts listed here and no content row in this
	/// store, by candidate or confirmed uuid.
	pub async fn content_sidecar_orphans(&self) -> Result<Vec<Uuid>> {
		if !self.has_content_sidecar_table().await? {
			return Ok(Vec::new());
		}
		Ok(sqlx::query_scalar(
			"SELECT DISTINCT s.content_uuid FROM content_sidecar s
				 WHERE NOT EXISTS (
					SELECT 1 FROM content c
					 WHERE c.candidate_uuid = s.content_uuid OR c.uuid = s.content_uuid
				 )",
		)
		.fetch_all(self.pool())
		.await?)
	}

	/// Whether this store has a content row for `content_uuid`, by candidate
	/// or confirmed uuid.
	pub async fn holds_content(&self, content_uuid: Uuid) -> Result<bool> {
		let found: Option<i64> = sqlx::query_scalar(
			"SELECT 1 FROM content WHERE candidate_uuid = ? OR uuid = ? LIMIT 1",
		)
		.bind(content_uuid)
		.bind(content_uuid)
		.fetch_optional(self.pool())
		.await?;
		Ok(found.is_some())
	}

	/// A replica copied from an older owner has no table, and a read-only
	/// handle cannot add one.
	async fn has_content_sidecar_table(&self) -> Result<bool> {
		let found: Option<i64> = sqlx::query_scalar(
			"SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'content_sidecar'",
		)
		.fetch_optional(self.pool())
		.await?;
		Ok(found.is_some())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{filesystem_schema, SourceManager};

	async fn store() -> (tempfile::TempDir, SourceDb) {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().to_path_buf());
		manager
			.create("s", &filesystem_schema())
			.await
			.expect("source created");
		let db = manager.open("s").await.expect("open");
		(dir, db)
	}

	fn row(content_uuid: Uuid, kind: &str, size: u64) -> ContentSidecarRow {
		ContentSidecarRow {
			content_uuid,
			producer: "core".into(),
			kind: kind.into(),
			variant: "default".into(),
			format: "json".into(),
			producer_version: Some("1".into()),
			size,
			checksum: Some("abc".into()),
			created_at: 1,
		}
	}

	#[tokio::test]
	async fn a_row_is_replaced_under_its_key_and_deleted_by_it() {
		let (_dir, db) = store().await;
		let content = Uuid::now_v7();
		db.put_content_sidecar(&row(content, "shots", 10))
			.await
			.unwrap();
		db.put_content_sidecar(&row(content, "shots", 20))
			.await
			.unwrap();
		db.put_content_sidecar(&row(content, "transcript", 5))
			.await
			.unwrap();
		let rows = db.content_sidecars(content).await.unwrap();
		assert_eq!(rows.len(), 2);
		assert_eq!(rows[0].size, 20);
		assert_eq!(db.content_sidecar_totals().await.unwrap(), (2, 25));

		assert!(db.delete_content_sidecar(&rows[0].key()).await.unwrap());
		assert!(!db.delete_content_sidecar(&rows[0].key()).await.unwrap());
		assert_eq!(db.content_sidecars(content).await.unwrap().len(), 1);
	}

	#[tokio::test]
	async fn orphans_are_content_this_store_no_longer_holds() {
		let (_dir, db) = store().await;
		let held = Uuid::now_v7();
		let gone = Uuid::now_v7();
		sqlx::query("INSERT INTO content (uuid, candidate_uuid, sampled_hash) VALUES (?, ?, 'x')")
			.bind(Uuid::now_v7())
			.bind(held)
			.execute(db.pool())
			.await
			.unwrap();
		db.put_content_sidecar(&row(held, "shots", 1))
			.await
			.unwrap();
		db.put_content_sidecar(&row(gone, "shots", 1))
			.await
			.unwrap();

		assert_eq!(db.content_sidecar_orphans().await.unwrap(), vec![gone]);
		assert!(db.holds_content(held).await.unwrap());
		assert!(!db.holds_content(gone).await.unwrap());
	}
}
