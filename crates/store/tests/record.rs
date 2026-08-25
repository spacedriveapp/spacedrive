//! End-to-end checks against real SQLite stores: write records and facets,
//! then read them back through the record table.

use sd_store::db::{OverlayEvidence, Stamp};
use sd_store::record::facet_table;
use sd_store::record::ContentIdentity;
use sd_store::schema::parser;
use sd_store::source::SourceManager;
use sd_store::uuid_for;
use serde_json::json;
use uuid::Uuid;

/// Rebind evidence for an adapter record, whose external id is stable at
/// the source and which has no content hash.
fn evidence(external_id: &str) -> OverlayEvidence {
	OverlayEvidence {
		type_: "note".to_string(),
		external_id: external_id.to_string(),
		content_uuid: None,
	}
}

fn stamp(hlc: &str) -> Stamp {
	Stamp {
		hlc: hlc.to_string(),
		device_uuid: Uuid::nil(),
	}
}

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

	async fn open(&self) -> sd_store::db::SourceDb {
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
			.bind(uuid)
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
	.bind(uuid)
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
		.bind(first)
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

	let parent: Option<Uuid> = sqlx::query_scalar("SELECT parent_uuid FROM record WHERE uuid = ?")
		.bind(note)
		.fetch_one(db.pool())
		.await
		.expect("parent");

	assert_eq!(parent, Some(folder));
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

	let uuid_a: Uuid = sqlx::query_scalar(
		"SELECT uuid FROM record WHERE type = 'note' AND external_id = 'note-1'",
	)
	.fetch_one(db.pool())
	.await
	.expect("uuid");

	let neighbors = db.neighbors(uuid_a, None).await.expect("neighbors");
	assert_eq!(neighbors.len(), 1);
	assert_eq!(neighbors[0].external_id, "note-2");
	assert!(neighbors[0].outgoing);

	db.unlink("note", "note-1", "note", "note-2")
		.await
		.expect("unlink");
	assert!(db.neighbors(uuid_a, None).await.expect("after").is_empty());
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
async fn overlays_merge_and_key_on_the_record() {
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

	let note = db
		.upsert(
			"note",
			"note-1",
			&json!({ "title": "Q2", "body": "revenue" }),
		)
		.await
		.expect("upsert");

	db.set_overlay(
		note,
		&evidence("note-1"),
		&stamp("1"),
		&json!({ "starred": true, "tag": "work" }),
	)
	.await
	.expect("set");

	// Merge semantics: absent keys are preserved, explicit null clears.
	let merged = db
		.set_overlay(
			note,
			&evidence("note-1"),
			&stamp("2"),
			&json!({ "tag": serde_json::Value::Null, "note": "hi" }),
		)
		.await
		.expect("merge");

	assert_eq!(merged["starred"], json!(true));
	assert_eq!(merged["note"], json!("hi"));
	assert!(merged.get("tag").is_none(), "null clears the field");

	// The record uuid is the key, so a re-ingest that keeps the uuid keeps the
	// assertions with no rebind needed.
	db.upsert("note", "note-1", &json!({ "title": "Q3", "body": "again" }))
		.await
		.expect("re-upsert");
	assert_eq!(
		db.get_overlay(note).await.expect("get")["starred"],
		json!(true)
	);
}

#[tokio::test]
async fn assertions_survive_a_record_re_minting_its_uuid() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let note = db
		.upsert(
			"note",
			"note-1",
			&json!({ "title": "Q2", "body": "revenue" }),
		)
		.await
		.expect("upsert");
	db.set_overlay(
		note,
		&evidence("note-1"),
		&stamp("1"),
		&json!({ "starred": true }),
	)
	.await
	.expect("set");

	// Dropping the record and re-ingesting mints a fresh uuid. The assertion
	// is left holding a uuid nothing answers to, which is what the evidence
	// columns are for.
	db.delete("note", "note-1").await.expect("delete");
	let reborn = db
		.upsert("note", "note-1", &json!({ "title": "Q3", "body": "again" }))
		.await
		.expect("re-upsert");
	assert_ne!(reborn, note, "a deleted record does not keep its uuid");
	assert!(db
		.get_overlay(reborn)
		.await
		.expect("get")
		.get("starred")
		.is_none());

	assert_eq!(db.rebind_overlays().await.expect("rebind"), 1);
	assert_eq!(
		db.get_overlay(reborn).await.expect("get")["starred"],
		json!(true)
	);
}

#[tokio::test]
async fn search_hits_carry_their_overlay() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let note = db
		.upsert(
			"note",
			"note-1",
			&json!({ "title": "Quarterly report", "body": "revenue is up" }),
		)
		.await
		.expect("upsert");
	db.upsert(
		"note",
		"note-2",
		&json!({ "title": "Quarterly plan", "body": "revenue targets" }),
	)
	.await
	.expect("upsert");

	db.set_overlay(
		note,
		&evidence("note-1"),
		&stamp("1"),
		&json!({ "starred": true }),
	)
	.await
	.expect("set");

	let hits = db.fts_search("quarterly", 10, None).await.expect("search");
	let ids: Vec<Uuid> = hits.iter().map(|h| h.id).collect();
	let overlays = db.overlays_for(&ids).await.expect("overlays");

	assert_eq!(overlays.len(), 1, "only the annotated record has one");
	assert_eq!(overlays[&note]["starred"], json!(true));
}

#[tokio::test]
async fn one_set_of_bytes_is_one_content_row() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let a = db
		.upsert("note", "note-1", &json!({ "title": "A" }))
		.await
		.expect("a");
	let b = db
		.upsert("note", "note-2", &json!({ "title": "B" }))
		.await
		.expect("b");

	let identity = ContentIdentity {
		sampled_hash: Some("sampled-1".to_string()),
		size: Some(1024),
		..Default::default()
	};
	let first = db
		.set_content_identity(a, &identity)
		.await
		.expect("first identity");
	let second = db
		.set_content_identity(b, &identity)
		.await
		.expect("second identity");

	assert_eq!(first, second, "two copies of one file share a content row");

	let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM content")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(rows, 1);

	let stored: Uuid = sqlx::query_scalar("SELECT uuid FROM content WHERE id = ?")
		.bind(first)
		.fetch_one(db.pool())
		.await
		.expect("uuid");
	assert_eq!(
		stored,
		uuid_for("sampled-1"),
		"the id derives from the hash, so another machine computes the same one"
	);
}

#[tokio::test]
async fn the_integrity_tier_renames_the_content_without_moving_the_row() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let note = db
		.upsert("note", "note-1", &json!({ "title": "A" }))
		.await
		.expect("note");

	let candidate = db
		.set_content_identity(
			note,
			&ContentIdentity {
				sampled_hash: Some("sampled-1".to_string()),
				..Default::default()
			},
		)
		.await
		.expect("candidate");

	let confirmed = db
		.set_content_identity(
			note,
			&ContentIdentity {
				sampled_hash: Some("sampled-1".to_string()),
				integrity_hash: Some("integrity-1".to_string()),
				..Default::default()
			},
		)
		.await
		.expect("confirmed");

	assert_eq!(
		candidate, confirmed,
		"the row the record points at is stable"
	);

	let stored: Uuid = sqlx::query_scalar("SELECT uuid FROM content WHERE id = ?")
		.bind(confirmed)
		.fetch_one(db.pool())
		.await
		.expect("uuid");
	assert_eq!(stored, uuid_for("integrity-1"));

	// A later write carrying only the cheap hash must not walk the identity
	// back down to a guess.
	db.set_content_identity(
		note,
		&ContentIdentity {
			sampled_hash: Some("sampled-1".to_string()),
			..Default::default()
		},
	)
	.await
	.expect("re-sampled");

	let after: Uuid = sqlx::query_scalar("SELECT uuid FROM content WHERE id = ?")
		.bind(confirmed)
		.fetch_one(db.pool())
		.await
		.expect("uuid");
	assert_eq!(after, uuid_for("integrity-1"));
}

#[tokio::test]
async fn the_file_facet_hangs_off_the_same_record_table() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	sqlx::raw_sql(sd_store::FILE_SCHEMA)
		.execute(db.pool())
		.await
		.expect("file schema applies");

	let key: String =
		sqlx::query_scalar("SELECT type FROM pragma_table_info('facet_file') WHERE name = ?")
			.bind("record_uuid")
			.fetch_one(db.pool())
			.await
			.expect("record_uuid column");
	assert_eq!(key, "BLOB");

	let indexed: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pragma_index_list('facet_file') WHERE name = 'idx_facet_file_inode'",
	)
	.fetch_one(db.pool())
	.await
	.expect("inode index");
	assert_eq!(indexed, 1, "the rebind procedure looks files up by inode");
}
