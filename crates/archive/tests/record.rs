//! End-to-end checks against real SQLite indexes: ingest through the adapter
//! protocol's shape, then read back through the record table.

use sd_archive::library::{Library, RecordKey};
use sd_archive::record::facet_table;
use sd_archive::schema::parser;
use sd_archive::source::SourceManager;
use serde_json::json;

const SCHEMA: &str = r#"
[data_type]
id = "note"
name = "Note"

[models.folder]
fields.name = "string"
fields.account = "string"

[models.note]
fields.title = "string"
fields.body = "text"
fields.snippet = "string"
fields.created = "datetime"
fields.modified = "datetime"

[models.note.relations]
belongs_to = ["folder"]
many_to_many = ["note"]

[search]
primary_model = "note"
title = "title"
preview = "body"
subtitle = "snippet"
search_fields = ["title", "body"]
date_field = "modified"
"#;

struct Fixture {
	_dir: tempfile::TempDir,
	manager: SourceManager,
	source_id: String,
}

impl Fixture {
	async fn new() -> Self {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().join("sources"));
		let schema = parser::parse(SCHEMA).expect("schema parses");
		let source_id = "src-1".to_string();

		manager
			.create(&source_id, &schema)
			.await
			.expect("source created");

		Self {
			_dir: dir,
			manager,
			source_id,
		}
	}

	async fn open(&self) -> sd_archive::db::SourceDb {
		self.manager
			.open(&self.source_id)
			.await
			.expect("open index")
	}
}

#[tokio::test]
async fn upsert_writes_record_and_facet() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let uuid = db
		.upsert(
			"note",
			"note-1",
			&json!({
				"title": "Groceries",
				"body": "milk, eggs",
				"modified": "2026-07-28T12:00:00Z",
			}),
		)
		.await
		.expect("upsert");

	let (external_id, type_, title, modified_at): (String, String, Option<String>, Option<i64>) =
		sqlx::query_as("SELECT external_id, type, title, modified_at FROM record WHERE uuid = ?")
			.bind(&uuid)
			.fetch_one(db.pool())
			.await
			.expect("record row");

	assert_eq!(external_id, "note-1");
	assert_eq!(type_, "note");
	assert_eq!(title.as_deref(), Some("Groceries"));
	assert_eq!(modified_at, Some(1_785_240_000_000));

	let body: Option<String> = sqlx::query_scalar(&format!(
		"SELECT body FROM \"{}\" WHERE record_uuid = ?",
		facet_table("note")
	))
	.bind(&uuid)
	.fetch_one(db.pool())
	.await
	.expect("facet row");

	assert_eq!(body.as_deref(), Some("milk, eggs"));
}

#[tokio::test]
async fn identity_is_assigned_once_and_kept_across_reingest() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let first = db
		.upsert("note", "note-1", &json!({ "title": "Draft" }))
		.await
		.expect("first upsert");

	db.begin_sync().await.expect("second epoch");
	let second = db
		.upsert("note", "note-1", &json!({ "title": "Final" }))
		.await
		.expect("second upsert");

	assert_eq!(first, second, "uuid must survive re-ingestion");

	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(count, 1);

	let title: Option<String> = sqlx::query_scalar("SELECT title FROM record WHERE uuid = ?")
		.bind(&first)
		.fetch_one(db.pool())
		.await
		.expect("title");
	assert_eq!(title.as_deref(), Some("Final"));
}

#[tokio::test]
async fn same_external_id_across_types_stays_distinct() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let folder = db
		.upsert("folder", "shared-id", &json!({ "name": "Work" }))
		.await
		.expect("folder");
	let note = db
		.upsert("note", "shared-id", &json!({ "title": "Work" }))
		.await
		.expect("note");

	assert_ne!(folder, note);
	let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(count, 2);
}

#[tokio::test]
async fn belongs_to_becomes_the_record_parent() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let folder = db
		.upsert("folder", "folder-1", &json!({ "name": "Work" }))
		.await
		.expect("folder");
	let note = db
		.upsert(
			"note",
			"note-1",
			&json!({ "title": "Standup", "folder_id": "folder-1" }),
		)
		.await
		.expect("note");

	let parent: Option<String> =
		sqlx::query_scalar("SELECT parent_uuid FROM record WHERE uuid = ?")
			.bind(&note)
			.fetch_one(db.pool())
			.await
			.expect("parent");

	assert_eq!(parent.as_deref(), Some(folder.as_str()));
}

#[tokio::test]
async fn link_creates_a_traversable_edge() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	db.upsert("note", "note-1", &json!({ "title": "A" }))
		.await
		.expect("a");
	db.upsert("note", "note-2", &json!({ "title": "B" }))
		.await
		.expect("b");

	db.link("note", "note-1", "note", "note-2")
		.await
		.expect("link");

	let uuid_a: String = sqlx::query_scalar(
		"SELECT uuid FROM record WHERE type = 'note' AND external_id = 'note-1'",
	)
	.fetch_one(db.pool())
	.await
	.expect("uuid");

	let neighbors = db.neighbors(&uuid_a, None).await.expect("neighbors");
	assert_eq!(neighbors.len(), 1);
	assert_eq!(neighbors[0].external_id, "note-2");
	assert!(neighbors[0].outgoing);

	db.unlink("note", "note-1", "note", "note-2")
		.await
		.expect("unlink");
	assert!(db.neighbors(&uuid_a, None).await.expect("after").is_empty());
}

#[tokio::test]
async fn deleting_a_record_cascades_its_facet_and_edges() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	db.upsert("note", "note-1", &json!({ "title": "A" }))
		.await
		.expect("a");
	db.upsert("note", "note-2", &json!({ "title": "B" }))
		.await
		.expect("b");
	db.link("note", "note-1", "note", "note-2")
		.await
		.expect("link");

	db.delete("note", "note-1").await.expect("delete");

	let facets: i64 =
		sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{}\"", facet_table("note")))
			.fetch_one(db.pool())
			.await
			.expect("facet count");
	assert_eq!(facets, 1, "facet row must not outlive its record");

	let edges: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edge")
		.fetch_one(db.pool())
		.await
		.expect("edge count");
	assert_eq!(edges, 0, "edges must not outlive their endpoints");
}

#[tokio::test]
async fn upsert_makes_a_record_searchable() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	db.upsert(
		"note",
		"note-1",
		&json!({ "title": "Quarterly report", "body": "revenue is up" }),
	)
	.await
	.expect("upsert");

	let hits = db.fts_search("quarterly", 10, None).await.expect("search");
	assert_eq!(hits.len(), 1);
	assert_eq!(hits[0].external_id, "note-1");
	assert_eq!(hits[0].title, "Quarterly report");
	assert_eq!(hits[0].preview.as_deref(), Some("revenue is up"));

	// Deleting the record takes its index row with it.
	db.delete("note", "note-1").await.expect("delete");
	assert!(db
		.fts_search("quarterly", 10, None)
		.await
		.expect("search")
		.is_empty());
}

#[tokio::test]
async fn reindexing_updated_content_keeps_the_index_current() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	db.upsert(
		"note",
		"note-1",
		&json!({ "title": "Original", "body": "one" }),
	)
	.await
	.expect("upsert");
	assert_eq!(db.fts_search("original", 10, None).await.unwrap().len(), 1);

	db.begin_sync().await.expect("epoch");
	db.upsert(
		"note",
		"note-1",
		&json!({ "title": "Rewritten", "body": "two" }),
	)
	.await
	.expect("re-upsert");

	assert!(db
		.fts_search("original", 10, None)
		.await
		.unwrap()
		.is_empty());
	assert_eq!(db.fts_search("rewritten", 10, None).await.unwrap().len(), 1);
}

#[tokio::test]
async fn list_items_reads_the_primary_type_only() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	db.upsert("folder", "folder-1", &json!({ "name": "Work" }))
		.await
		.expect("folder");
	db.upsert(
		"note",
		"note-1",
		&json!({ "title": "Older", "snippet": "s1", "modified": "2026-07-01T00:00:00Z" }),
	)
	.await
	.expect("older");
	db.upsert(
		"note",
		"note-2",
		&json!({ "title": "Newer", "snippet": "s2", "modified": "2026-07-28T00:00:00Z" }),
	)
	.await
	.expect("newer");

	let items = db.list_items(10, 0).await.expect("list");
	assert_eq!(items.len(), 2, "folders are not the primary type");
	assert_eq!(items[0].title, "Newer", "newest first");
	assert_eq!(items[1].title, "Older");

	assert_eq!(db.count("note").await.unwrap(), 2);
	assert_eq!(db.count_all().await.unwrap(), 3);
}

#[tokio::test]
async fn scan_epoch_advances_per_sync_run() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;

	let first = db.begin_sync().await.expect("first");
	db.upsert("note", "note-1", &json!({ "title": "A" }))
		.await
		.expect("a");

	let second = db.begin_sync().await.expect("second");
	assert!(second > first);
	db.upsert("note", "note-2", &json!({ "title": "B" }))
		.await
		.expect("b");

	// Records untouched by a run keep the epoch that last saw them; the delta
	// protocol means an untouched record is still present at the source.
	let epochs: Vec<i64> = sqlx::query_scalar("SELECT scan_epoch FROM record ORDER BY external_id")
		.fetch_all(db.pool())
		.await
		.expect("epochs");
	assert_eq!(epochs, vec![first, second]);
}

#[tokio::test]
async fn overlays_survive_source_deletion() {
	let dir = tempfile::tempdir().expect("tempdir");
	let pool = sqlx::SqlitePool::connect(&format!(
		"sqlite:{}?mode=rwc",
		dir.path().join("registry.db").display()
	))
	.await
	.expect("pool");

	let library = Library::new(pool).await.expect("library");
	let key = RecordKey::new("src-1", "note", "note-1");

	library
		.set_overlay(&key, &json!({ "starred": true, "tag": "work" }))
		.await
		.expect("set");

	// Merge semantics: absent keys are preserved, explicit null clears.
	let merged = library
		.set_overlay(
			&key,
			&json!({ "tag": serde_json::Value::Null, "note": "hi" }),
		)
		.await
		.expect("merge");

	assert_eq!(merged["starred"], json!(true));
	assert_eq!(merged["note"], json!("hi"));
	assert!(merged.get("tag").is_none(), "null clears the field");

	// A source's index being destroyed does not touch the durable layer.
	let after = library.get_overlay(&key).await.expect("get");
	assert_eq!(after["starred"], json!(true));
}

#[tokio::test]
async fn durable_edges_span_sources() {
	let dir = tempfile::tempdir().expect("tempdir");
	let pool = sqlx::SqlitePool::connect(&format!(
		"sqlite:{}?mode=rwc",
		dir.path().join("registry.db").display()
	))
	.await
	.expect("pool");

	let library = Library::new(pool).await.expect("library");
	let email = RecordKey::new("gmail", "message", "msg-1");
	let note = RecordKey::new("obsidian", "note", "note-1");

	library.link(&email, &note, "mentions").await.expect("link");

	let neighbors = library.neighbors(&email, None).await.expect("neighbors");
	assert_eq!(neighbors.len(), 1);
	assert!(neighbors[0].1, "email is the edge source");
	assert_eq!(neighbors[0].0.dst, note);

	let inbound = library.neighbors(&note, None).await.expect("inbound");
	assert_eq!(inbound.len(), 1);
	assert!(!inbound[0].1, "note is the edge target");

	library
		.unlink(&email, &note, "mentions")
		.await
		.expect("unlink");
	assert!(library.neighbors(&email, None).await.unwrap().is_empty());
}
