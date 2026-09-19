//! The store revision against real stores: what moves it, and what has to
//! leave it where it was.

use sd_store::file::{FileKind, FileWrite, Ledger, Observation, Watermark};
use sd_store::schema::parser;
use sd_store::{
	filesystem_schema, normalize_tag_path, slug_for_path, Revision, SourceDb, SourceManager, Stamp,
	TagAssertion, TagDefinition,
};
use serde_json::json;
use uuid::Uuid;

const NOTES: &str = r#"
[data_type]
id = "note"
name = "Note"

[models.note]
fields.title = "string"
fields.body = "text"

[search]
primary_model = "note"
title = "title"
preview = "body"
search_fields = ["title", "body"]
"#;

/// [`NOTES`] with a field declared after the store was created.
const NOTES_WIDENED: &str = r#"
[data_type]
id = "note"
name = "Note"

[models.note]
fields.title = "string"
fields.body = "text"
fields.pinned = "string"

[search]
primary_model = "note"
title = "title"
preview = "body"
search_fields = ["title", "body"]
"#;

struct Fixture {
	dir: tempfile::TempDir,
	manager: SourceManager,
}

impl Fixture {
	async fn files() -> Self {
		Self::with_schema(&filesystem_schema()).await
	}

	async fn notes() -> Self {
		Self::with_schema(&parser::parse(NOTES).expect("schema parses")).await
	}

	async fn with_schema(schema: &sd_store::DataTypeSchema) -> Self {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().join("sources"));
		manager
			.create("source-1", schema)
			.await
			.expect("source created");
		Self { dir, manager }
	}

	async fn open(&self) -> SourceDb {
		self.manager.open("source-1").await.expect("open")
	}
}

async fn revision(db: &SourceDb) -> Revision {
	db.revision().await.expect("revision")
}

fn observe(path: &str, size: i64, mtime: i64) -> Observation {
	Observation {
		external_id: path.to_string(),
		kind: FileKind::File,
		name: path.to_string(),
		size,
		mtime,
		created: None,
		accessed: None,
		inode: None,
		mode: Some(0o644),
		uid: None,
		gid: None,
		link_target: None,
		extension: path.rsplit_once('.').map(|(_, e)| e.to_string()),
		is_hidden: false,
		identity: None,
	}
}

/// Resolve observations against what the store holds and write the result,
/// the way a walk does. Returns each observation's record uuid.
async fn walk(db: &SourceDb, observations: &[Observation]) -> Vec<Uuid> {
	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let writes: Vec<FileWrite> = observations
		.iter()
		.cloned()
		.map(|observation| FileWrite {
			resolution: ledger.resolve(&observation),
			parent_uuid: None,
			observation,
		})
		.collect();
	db.apply_files(&writes, &[], &[], None)
		.await
		.expect("apply");
	writes.iter().map(FileWrite::uuid).collect()
}

fn hlc(timestamp: u64) -> String {
	format!("{timestamp:016x}-{:016x}-{}", 0, Uuid::nil())
}

#[tokio::test]
async fn a_store_reads_untracked_until_a_writer_opens_it() {
	let fixture = Fixture::files().await;
	let reader = fixture
		.manager
		.open_read_only("source-1")
		.await
		.expect("read-only open");
	assert_eq!(revision(&reader).await, Revision::UNTRACKED);

	let writer = fixture.open().await;
	let opened = revision(&writer).await;
	assert_ne!(opened.store_id, Uuid::nil(), "a writer draws a store id");
	assert_eq!(opened.value, 0);

	let reopened = fixture.open().await;
	assert_eq!(
		revision(&reopened).await,
		opened,
		"reopening keeps the store id and the count"
	);
	assert_eq!(
		revision(&reader).await,
		opened,
		"a read-only handle sees the writer's revision"
	);
}

#[tokio::test]
async fn file_changes_move_the_revision_and_nothing_else_does() {
	let fixture = Fixture::files().await;
	let db = fixture.open().await;
	let files = [observe("a.txt", 10, 1), observe("b.txt", 20, 1)];

	let uuids = walk(&db, &files).await;
	let written = revision(&db).await.value;
	assert!(written > 0, "new files move the revision");

	walk(&db, &files).await;
	db.apply_files(
		&[],
		&[],
		&[],
		Some(Watermark {
			key: "walk",
			value: "b.txt",
		}),
	)
	.await
	.expect("watermark");
	assert_eq!(
		revision(&db).await.value,
		written,
		"an unchanged walk and a watermark leave the revision"
	);

	walk(&db, &[observe("a.txt", 11, 2)]).await;
	let changed = revision(&db).await.value;
	assert!(changed > written, "a changed file moves the revision");

	db.apply_files(&[], &[uuids[1]], &[], None)
		.await
		.expect("remove");
	assert!(
		revision(&db).await.value > changed,
		"a removal moves the revision"
	);
}

#[tokio::test]
async fn re_putting_an_unchanged_record_leaves_the_revision() {
	let fixture = Fixture::notes().await;
	let db = fixture.open().await;
	let note = json!({ "title": "Groceries", "body": "eggs" });

	db.upsert("note", "n1", &note).await.expect("put");
	let written = revision(&db).await.value;

	db.begin_sync().await.expect("epoch");
	db.upsert("note", "n1", &note).await.expect("re-put");
	assert_eq!(
		revision(&db).await.value,
		written,
		"a new epoch over identical fields leaves the revision"
	);

	db.upsert(
		"note",
		"n1",
		&json!({ "title": "Groceries", "body": "eggs, milk" }),
	)
	.await
	.expect("edit");
	assert!(
		revision(&db).await.value > written,
		"a changed field moves the revision"
	);
}

#[tokio::test]
async fn tags_move_the_revision() {
	let fixture = Fixture::notes().await;
	let db = fixture.open().await;
	let record = db
		.upsert("note", "n1", &json!({ "title": "Groceries" }))
		.await
		.expect("put");
	let before = revision(&db).await.value;

	let path = normalize_tag_path("errands").expect("valid path");
	let definition = TagDefinition {
		uuid: Uuid::now_v7(),
		slug_id: slug_for_path(&path),
		path,
		color: None,
		icon: None,
		updated_hlc: hlc(1),
		origin_device: Uuid::nil(),
	};
	db.upsert_tag_definitions(std::slice::from_ref(&definition))
		.await
		.expect("define");
	let defined = revision(&db).await.value;
	assert!(defined > before, "a new definition moves the revision");

	db.append_tag_assertions(&[TagAssertion {
		tag_uuid: definition.uuid,
		record_uuid: record,
		external_id: Some("n1".to_string()),
		content_uuid: None,
		asserted: true,
		stamp: Stamp {
			hlc: hlc(2),
			device_uuid: Uuid::nil(),
		},
	}])
	.await
	.expect("assert");
	assert!(
		revision(&db).await.value > defined,
		"an assertion moves the revision"
	);
}

#[tokio::test]
async fn a_facet_column_added_later_is_compared() {
	let fixture = Fixture::notes().await;
	let note = json!({ "title": "Groceries", "body": "eggs" });
	fixture
		.open()
		.await
		.upsert("note", "n1", &note)
		.await
		.expect("put");

	let widened = parser::parse(NOTES_WIDENED).expect("schema parses");
	let (db, _) = fixture
		.manager
		.open_with_migration("source-1", &widened)
		.await
		.expect("migrate");
	let before = revision(&db).await.value;

	db.upsert(
		"note",
		"n1",
		&json!({ "title": "Groceries", "body": "eggs", "pinned": "yes" }),
	)
	.await
	.expect("pin");
	assert!(
		revision(&db).await.value > before,
		"a change in the new column moves the revision"
	);
}

#[tokio::test]
async fn reopening_an_unchanged_store_rewrites_no_triggers() {
	let fixture = Fixture::files().await;
	let first = fixture.open().await;
	let before: i64 = sqlx::query_scalar("PRAGMA schema_version")
		.fetch_one(first.pool())
		.await
		.expect("schema version");
	first.pool().close().await;

	let second = fixture.open().await;
	let after: i64 = sqlx::query_scalar("PRAGMA schema_version")
		.fetch_one(second.pool())
		.await
		.expect("schema version");
	assert_eq!(before, after, "an unchanged store reopens without DDL");
}

#[tokio::test]
async fn a_delivered_copy_carries_the_revision() {
	let fixture = Fixture::files().await;
	let db = fixture.open().await;
	walk(&db, &[observe("a.txt", 10, 1)]).await;
	let owner = revision(&db).await;

	let copy = fixture.dir.path().join("copy.db");
	sqlx::query("VACUUM INTO ?")
		.bind(copy.to_string_lossy().into_owned())
		.execute(db.pool())
		.await
		.expect("export");
	let replica = SourceManager::open_file_read_only(&copy)
		.await
		.expect("open copy");
	assert_eq!(revision(&replica).await, owner);
}
