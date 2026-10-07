//! The store schema version against real files: who writes it, who reads it,
//! and that a `VACUUM INTO` copy keeps it.

use std::path::Path;

use sd_store::{filesystem_schema, migrate, uuid_for, SourceManager, SCHEMA_VERSION};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use uuid::Uuid;

struct Fixture {
	dir: tempfile::TempDir,
	manager: SourceManager,
}

impl Fixture {
	async fn new() -> Self {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().join("sources"));
		manager
			.create("source-1", &filesystem_schema())
			.await
			.expect("source created");
		Self { dir, manager }
	}

	fn db_path(&self) -> std::path::PathBuf {
		self.dir.path().join("sources/source-1/data.db")
	}
}

/// A raw writable connection to a store file, outside the manager, for
/// setting up shapes the manager would never produce.
async fn raw(path: &Path) -> SqlitePool {
	SqlitePoolOptions::new()
		.max_connections(1)
		.connect_with(
			SqliteConnectOptions::new()
				.filename(path)
				.create_if_missing(true),
		)
		.await
		.expect("raw pool")
}

#[tokio::test]
async fn a_fresh_store_carries_the_current_version() {
	let fixture = Fixture::new().await;
	let pool = raw(&fixture.db_path()).await;
	assert_eq!(
		migrate::version(&pool).await.expect("version"),
		SCHEMA_VERSION
	);

	let db = fixture.manager.open("source-1").await.expect("open");
	assert_eq!(db.schema_version(), SCHEMA_VERSION);
	let reader = fixture
		.manager
		.open_read_only("source-1")
		.await
		.expect("open read-only");
	assert_eq!(reader.schema_version(), SCHEMA_VERSION);
}

#[tokio::test]
async fn vacuum_into_keeps_the_owners_version() {
	let fixture = Fixture::new().await;
	let pool = raw(&fixture.db_path()).await;
	// A value no build writes, so the copy cannot have arrived at it on its own.
	sqlx::query("PRAGMA user_version = 7")
		.execute(&pool)
		.await
		.expect("set version");

	let copy = fixture.dir.path().join("replica.db");
	sqlx::query("VACUUM INTO ?")
		.bind(copy.to_string_lossy().to_string())
		.execute(&pool)
		.await
		.expect("vacuum into");

	let replica = raw(&copy).await;
	assert_eq!(migrate::version(&replica).await.expect("version"), 7);
}

#[tokio::test]
async fn a_store_from_a_newer_build_is_refused() {
	let fixture = Fixture::new().await;
	let pool = raw(&fixture.db_path()).await;
	sqlx::query(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 1))
		.execute(&pool)
		.await
		.expect("set version");
	pool.close().await;

	let writer = fixture.manager.open("source-1").await;
	assert!(
		matches!(writer, Err(sd_store::Error::UnsupportedGeneration(_))),
		"a writer must not downgrade a store"
	);
	let reader = SourceManager::open_file_read_only(&fixture.db_path()).await;
	assert!(
		matches!(reader, Err(sd_store::Error::UnsupportedGeneration(_))),
		"a reader cannot know a shape newer than its own"
	);
}

/// The `content` table as every store wrote it before schema version 1:
/// one row per sampled hash, shared by every record carrying that hash.
const CONTENT_V0: &str = "\
CREATE TABLE content (
    id INTEGER PRIMARY KEY,
    uuid BLOB NOT NULL,
    sampled_hash TEXT UNIQUE,
    integrity_hash TEXT,
    size INTEGER,
    kind INTEGER
)";

/// Turn a fresh store into one written before schema version 1: the old
/// content table, and `user_version` 0. The record table did not change.
async fn downgrade_to_v0(path: &Path) -> SqlitePool {
	let pool = raw(path).await;
	for sql in [
		"DROP TABLE content",
		CONTENT_V0,
		"CREATE INDEX idx_content_uuid ON content(uuid)",
		"CREATE INDEX idx_content_integrity ON content(integrity_hash)",
		"PRAGMA user_version = 0",
	] {
		sqlx::query(sql).execute(&pool).await.expect(sql);
	}
	pool
}

async fn insert_record(pool: &SqlitePool, title: &str, content_id: Option<i64>) -> Uuid {
	let uuid = Uuid::now_v7();
	sqlx::query(
		"INSERT INTO record (uuid, type, title, content_id, scan_epoch) VALUES (?, 'file', ?, ?, 1)",
	)
	.bind(uuid)
	.bind(title)
	.bind(content_id)
	.execute(pool)
	.await
	.expect("record");
	uuid
}

async fn insert_v0_content(
	pool: &SqlitePool,
	id: i64,
	sampled: Option<&str>,
	integrity: Option<&str>,
) {
	let uuid = match (integrity, sampled) {
		(Some(hash), _) | (None, Some(hash)) => uuid_for(hash),
		(None, None) => Uuid::nil(),
	};
	sqlx::query(
		"INSERT INTO content (id, uuid, sampled_hash, integrity_hash, size) VALUES (?, ?, ?, ?, 10)",
	)
	.bind(id)
	.bind(uuid)
	.bind(sampled)
	.bind(integrity)
	.execute(pool)
	.await
	.expect("content row");
}

async fn insert_assertion(pool: &SqlitePool, record: Uuid, content: Option<Uuid>, hlc: &str) {
	sqlx::query(
		"INSERT INTO tag_assertion (tag_uuid, record_uuid, content_uuid, asserted, hlc, device_uuid)
		 VALUES (?, ?, ?, 1, ?, ?)",
	)
	.bind(Uuid::nil())
	.bind(record)
	.bind(content)
	.bind(hlc)
	.bind(Uuid::nil())
	.execute(pool)
	.await
	.expect("assertion");
	sqlx::query(
		"INSERT INTO record_overlay (record_uuid, type, external_id, content_uuid, fields, hlc, device_uuid)
		 VALUES (?, 'file', ?, ?, '{}', ?, ?)",
	)
	.bind(record)
	.bind(record.to_string())
	.bind(content)
	.bind(hlc)
	.bind(Uuid::nil())
	.execute(pool)
	.await
	.expect("overlay");
}

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct ContentRow {
	id: i64,
	uuid: Uuid,
	candidate_uuid: Option<Uuid>,
	sampled_hash: Option<String>,
	integrity_hash: Option<String>,
}

async fn content_rows(pool: &SqlitePool) -> Vec<ContentRow> {
	sqlx::query_as(
		"SELECT id, uuid, candidate_uuid, sampled_hash, integrity_hash FROM content ORDER BY id",
	)
	.fetch_all(pool)
	.await
	.expect("content rows")
}

/// Migration 1 against a store in the pre-version shape, holding every case
/// the plan names: a shared row carrying both hashes, two integrity-only
/// rows for one hash, a row nothing references, and a row with no hash.
#[tokio::test]
async fn migration_1_rebuilds_content_and_keeps_every_content_id() {
	let fixture = Fixture::new().await;
	let pool = downgrade_to_v0(&fixture.db_path()).await;

	// Row 1: two records share it, and one of them was read in full. The
	// store cannot say which, so neither keeps the integrity hash.
	insert_v0_content(&pool, 1, Some("s1"), Some("i1")).await;
	let a = insert_record(&pool, "a.bin", Some(1)).await;
	let b = insert_record(&pool, "b.bin", Some(1)).await;
	// Row 2: sampled only, already a candidate.
	insert_v0_content(&pool, 2, Some("s2"), None).await;
	let c = insert_record(&pool, "c.bin", Some(2)).await;
	// Rows 3 and 4: integrity only, the same hash twice. Each was reached by
	// its own record, so both stay confirmed and merge into one row.
	insert_v0_content(&pool, 3, None, Some("i3")).await;
	insert_v0_content(&pool, 4, None, Some("i3")).await;
	let d = insert_record(&pool, "d.bin", Some(3)).await;
	let e = insert_record(&pool, "e.bin", Some(4)).await;
	// Row 5: nothing points at it.
	insert_v0_content(&pool, 5, Some("s5"), Some("i5")).await;
	// Row 6: no hash at all. Its record goes back to the hashing queue.
	insert_v0_content(&pool, 6, None, None).await;
	let f = insert_record(&pool, "f.bin", Some(6)).await;
	let unhashed = insert_record(&pool, "g.bin", None).await;

	// Assertions: one anchored on a demoted record and keyed by its old
	// confirmed uuid, one keyed by the candidate's uuid, one record-only, and
	// one on a record whose row is not demoted.
	insert_assertion(&pool, a, Some(uuid_for("i1")), "1").await;
	insert_assertion(&pool, c, Some(uuid_for("s2")), "2").await;
	insert_assertion(&pool, b, None, "3").await;
	insert_assertion(&pool, d, Some(uuid_for("i3")), "4").await;
	pool.close().await;

	let db = fixture
		.manager
		.open("source-1")
		.await
		.expect("open migrates");
	assert_eq!(db.schema_version(), SCHEMA_VERSION);
	assert_eq!(
		migrate::version(db.pool()).await.expect("version"),
		SCHEMA_VERSION
	);

	let rows = content_rows(db.pool()).await;
	assert_eq!(
		rows,
		vec![
			ContentRow {
				id: 1,
				uuid: uuid_for("s1"),
				candidate_uuid: Some(uuid_for("s1")),
				sampled_hash: Some("s1".into()),
				integrity_hash: None,
			},
			ContentRow {
				id: 2,
				uuid: uuid_for("s2"),
				candidate_uuid: Some(uuid_for("s2")),
				sampled_hash: Some("s2".into()),
				integrity_hash: None,
			},
			ContentRow {
				id: 3,
				uuid: uuid_for("i3"),
				candidate_uuid: None,
				sampled_hash: None,
				integrity_hash: Some("i3".into()),
			},
		],
		"demoted, kept, merged; orphan and hashless rows gone"
	);

	// Every record's content_id still resolves to a row, or was cleared
	// because its row held nothing a hash job could not redo.
	let pointed: Vec<(Uuid, Option<i64>)> =
		sqlx::query_as("SELECT uuid, content_id FROM record ORDER BY title")
			.fetch_all(db.pool())
			.await
			.expect("records");
	assert_eq!(
		pointed,
		vec![
			(a, Some(1)),
			(b, Some(1)),
			(c, Some(2)),
			(d, Some(3)),
			(e, Some(3)),
			(f, None),
			(unhashed, None),
		]
	);
	let dangling: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM record r WHERE r.content_id IS NOT NULL
		 AND NOT EXISTS (SELECT 1 FROM content c WHERE c.id = r.content_id)",
	)
	.fetch_one(db.pool())
	.await
	.expect("dangling");
	assert_eq!(dangling, 0);

	// Assertion rows: only the content key of rows anchored on a demoted
	// record changed, and only when it named the demoted uuid.
	for table in ["tag_assertion", "record_overlay"] {
		let keys: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as(&format!(
			"SELECT record_uuid, content_uuid, hlc FROM {table} ORDER BY hlc"
		))
		.fetch_all(db.pool())
		.await
		.expect("assertions");
		assert_eq!(
			keys,
			vec![
				(a, Some(uuid_for("s1")), "1".into()),
				(c, Some(uuid_for("s2")), "2".into()),
				(b, None, "3".into()),
				(d, Some(uuid_for("i3")), "4".into()),
			],
			"{table}"
		);
	}

	// The rebuilt table is written through, and the revision triggers came
	// back with it.
	let before = db.revision().await.expect("revision");
	db.set_content_identity(
		unhashed,
		&sd_store::ContentIdentity {
			sampled_hash: Some("s7".into()),
			size: Some(1),
			..Default::default()
		},
	)
	.await
	.expect("write through the new table");
	assert!(db.revision().await.expect("revision").value > before.value);

	// Reopening is a no-op.
	drop(db);
	let again = fixture.manager.open("source-1").await.expect("reopen");
	assert_eq!(content_rows(again.pool()).await.len(), 4);
}

#[tokio::test]
async fn a_read_only_open_of_an_old_local_store_migrates_it_first() {
	let fixture = Fixture::new().await;
	let pool = downgrade_to_v0(&fixture.db_path()).await;
	insert_v0_content(&pool, 1, Some("s1"), Some("i1")).await;
	insert_record(&pool, "a.bin", Some(1)).await;
	pool.close().await;

	let reader = fixture
		.manager
		.open_read_only("source-1")
		.await
		.expect("read-only open");
	assert_eq!(reader.schema_version(), SCHEMA_VERSION);
	let rows = content_rows(reader.pool()).await;
	assert_eq!(rows[0].candidate_uuid, Some(uuid_for("s1")));
	assert_eq!(rows[0].integrity_hash, None);
}

#[tokio::test]
async fn a_replica_below_the_current_version_is_read_in_its_own_shape() {
	let fixture = Fixture::new().await;
	let pool = downgrade_to_v0(&fixture.db_path()).await;
	insert_v0_content(&pool, 1, Some("s1"), Some("i1")).await;
	let a = insert_record(&pool, "a.bin", Some(1)).await;
	let replica_path = fixture.dir.path().join("replica.db");
	sqlx::query("VACUUM INTO ?")
		.bind(replica_path.to_string_lossy().to_string())
		.execute(&pool)
		.await
		.expect("vacuum into");
	pool.close().await;

	let replica = SourceManager::open_file_read_only(&replica_path)
		.await
		.expect("replica opens");
	assert_eq!(replica.schema_version(), 0, "the owner's version travels");
	assert_eq!(
		migrate::version(replica.pool()).await.expect("version"),
		0,
		"nothing migrated a file the owner has not"
	);

	let copies = sd_store::copies_of_content(&replica, uuid_for("i1"))
		.await
		.expect("lookup by the uuid the old shape holds");
	assert_eq!(copies.len(), 1);
	assert_eq!(copies[0].record_uuid, a);
	let tags = replica.tags_for_records(&[a]).await.expect("tags read");
	assert!(tags.is_empty());
}

/// Migration 1 on a store the size of a laptop's home folder: one million
/// records, half on shared both-hash rows and half on their own. Run with
/// `cargo test -p sd-store --test migrate -- --ignored migration_1_timing
/// --nocapture`; the elapsed time is printed.
#[tokio::test]
#[ignore]
async fn migration_1_timing_on_a_million_records() {
	let fixture = Fixture::new().await;
	let pool = downgrade_to_v0(&fixture.db_path()).await;
	let rows: i64 = std::env::var("SD_MIGRATE_ROWS")
		.ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(1_000_000);
	let mut tx = pool.begin().await.expect("tx");
	for id in 1..=rows {
		let sampled = format!("s{id}");
		let integrity = if id % 2 == 0 {
			Some(format!("i{id}"))
		} else {
			None
		};
		let uuid = uuid_for(integrity.as_deref().unwrap_or(&sampled));
		sqlx::query("INSERT INTO content (id, uuid, sampled_hash, integrity_hash, size) VALUES (?, ?, ?, ?, 10)")
			.bind(id).bind(uuid).bind(&sampled).bind(&integrity)
			.execute(&mut *tx).await.expect("content");
		sqlx::query("INSERT INTO record (uuid, type, title, content_id, scan_epoch) VALUES (?, 'file', ?, ?, 1)")
			.bind(Uuid::now_v7()).bind(format!("f{id}.bin")).bind(id)
			.execute(&mut *tx).await.expect("record");
		if id % 1000 == 0 {
			let record: Uuid = sqlx::query_scalar("SELECT uuid FROM record WHERE content_id = ?")
				.bind(id)
				.fetch_one(&mut *tx)
				.await
				.expect("record");
			sqlx::query("INSERT INTO tag_assertion (tag_uuid, record_uuid, content_uuid, asserted, hlc, device_uuid) VALUES (?, ?, ?, 1, ?, ?)")
				.bind(Uuid::nil()).bind(record).bind(uuid).bind(id.to_string()).bind(Uuid::nil())
				.execute(&mut *tx).await.expect("assertion");
		}
	}
	tx.commit().await.expect("commit");
	pool.close().await;

	let started = std::time::Instant::now();
	let db = fixture
		.manager
		.open("source-1")
		.await
		.expect("open migrates");
	let elapsed = started.elapsed();
	assert_eq!(db.schema_version(), SCHEMA_VERSION);
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(count, rows);
	println!("migration 1 over {rows} content rows and records: {elapsed:?}");
}
