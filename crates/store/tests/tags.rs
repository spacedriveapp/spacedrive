//! Tag semantics against real SQLite stores: definitions merge by stamp,
//! assertions append and dedupe, state resolves by HLC, content reaches every
//! copy, and the assertion half survives losing its generation.

use sd_store::db::Stamp;
use sd_store::record::ContentIdentity;
use sd_store::schema::parser;
use sd_store::source::SourceManager;
use sd_store::tags::{normalize_tag_path, slug_for_path, TagAssertion, TagDefinition};
use sd_store::uuid_for;
use serde_json::json;
use uuid::Uuid;

const SCHEMA: &str = r#"
[data_type]
id = "note"
name = "Note"

[models.note]
fields.title = "string"
fields.body = "text"
fields.snippet = "string"
fields.created = "datetime"
fields.modified = "datetime"

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
}

impl Fixture {
	async fn new() -> Self {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().join("sources"));
		Self { _dir: dir, manager }
	}

	async fn create(&self, source_id: &str) -> sd_store::db::SourceDb {
		let schema = parser::parse(SCHEMA).expect("schema parses");
		self.manager
			.create(source_id, &schema)
			.await
			.expect("source created");
		let db = self.manager.open(source_id).await.expect("open index");
		db.begin_sync().await.expect("epoch");
		db
	}
}

/// Sortable HLC text in the format `infra/sync/hlc.rs` emits.
fn hlc(timestamp: u64, device: Uuid) -> String {
	format!("{timestamp:016x}-{:016x}-{device}", 0)
}

fn stamp(timestamp: u64, device: Uuid) -> Stamp {
	Stamp {
		hlc: hlc(timestamp, device),
		device_uuid: device,
	}
}

fn definition(path: &str, timestamp: u64, device: Uuid) -> TagDefinition {
	let normalized = normalize_tag_path(path).expect("valid path");
	TagDefinition {
		uuid: Uuid::new_v4(),
		slug_id: slug_for_path(&normalized),
		path: normalized,
		color: Some("#ff5500".to_string()),
		icon: None,
		updated_hlc: hlc(timestamp, device),
		origin_device: device,
	}
}

fn apply(tag: &TagDefinition, record: Uuid, timestamp: u64, device: Uuid) -> TagAssertion {
	TagAssertion {
		tag_uuid: tag.uuid,
		record_uuid: record,
		external_id: None,
		content_uuid: None,
		asserted: true,
		stamp: stamp(timestamp, device),
	}
}

#[test]
fn paths_normalize_and_slugs_converge() {
	assert_eq!(
		normalize_tag_path(" Work / Clients ").expect("normalizes"),
		"Work/Clients"
	);
	assert!(normalize_tag_path("Work//Clients").is_err());
	assert!(normalize_tag_path("/Work").is_err());
	assert!(normalize_tag_path("Work/").is_err());
	assert!(normalize_tag_path("").is_err());

	// Case folds, and NFD composes to NFC, so the same concept typed on two
	// platforms lands on one slug.
	assert_eq!(slug_for_path("Caf\u{e9}"), slug_for_path("caf\u{e9}"));
	assert_eq!(slug_for_path("Caf\u{e9}"), slug_for_path("Cafe\u{301}"));
	// The full path is the concept: a shared leaf name must not merge.
	assert_ne!(
		slug_for_path("Work/Clients/Acme"),
		slug_for_path("Personal/Acme")
	);
}

#[tokio::test]
async fn definition_merge_takes_the_later_stamp() {
	let fixture = Fixture::new().await;
	let db = fixture.create("src-1").await;
	let device = Uuid::new_v4();

	let mut tag = definition("Work", 100, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("insert");

	// An older stamp arriving later loses the whole row.
	let mut stale = tag.clone();
	stale.path = "Stale".to_string();
	stale.updated_hlc = hlc(50, device);
	db.upsert_tag_definitions(&[stale]).await.expect("upsert");
	let listed = db.tag_definitions().await.expect("list");
	assert_eq!(listed.len(), 1);
	assert_eq!(listed[0].path, "Work");

	// A newer stamp wins, and the rename travels with a fresh slug.
	tag.path = "Work/Renamed".to_string();
	tag.slug_id = slug_for_path(&tag.path);
	tag.updated_hlc = hlc(200, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("upsert");
	let listed = db.tag_definitions().await.expect("list");
	assert_eq!(listed[0].path, "Work/Renamed");
	assert_eq!(listed[0].slug_id, slug_for_path("Work/Renamed"));
}

#[tokio::test]
async fn duplicate_delivery_inserts_nothing() {
	let fixture = Fixture::new().await;
	let db = fixture.create("src-1").await;
	let device = Uuid::new_v4();

	let note = db
		.upsert("note", "note-1", &json!({ "title": "Draft" }))
		.await
		.expect("upsert");
	let tag = definition("Work", 100, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("definition");

	let assertion = apply(&tag, note, 110, device);
	assert_eq!(
		db.append_tag_assertions(std::slice::from_ref(&assertion))
			.await
			.expect("append"),
		1
	);
	// Replay: the primary key absorbs the whole batch.
	assert_eq!(
		db.append_tag_assertions(std::slice::from_ref(&assertion))
			.await
			.expect("replay"),
		0
	);

	assert_eq!(
		db.records_with_tag(tag.uuid).await.expect("read"),
		vec![note]
	);
}

#[tokio::test]
async fn removal_wins_regardless_of_arrival_order() {
	let fixture = Fixture::new().await;
	let db = fixture.create("src-1").await;
	let device = Uuid::new_v4();

	let note = db
		.upsert("note", "note-1", &json!({ "title": "Draft" }))
		.await
		.expect("upsert");
	let tag = definition("Work", 100, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("definition");

	// The removal was authored later but delivered first; the apply arriving
	// afterwards must not resurrect the tag.
	let mut removal = apply(&tag, note, 300, device);
	removal.asserted = false;
	db.append_tag_assertions(&[removal]).await.expect("removal");
	db.append_tag_assertions(&[apply(&tag, note, 200, device)])
		.await
		.expect("late apply");

	assert!(db
		.records_with_tag(tag.uuid)
		.await
		.expect("read")
		.is_empty());
	assert!(db
		.tags_for_records(&[note])
		.await
		.expect("state")
		.is_empty());
}

#[tokio::test]
async fn content_tag_reaches_every_copy() {
	let fixture = Fixture::new().await;
	let db = fixture.create("src-1").await;
	let device = Uuid::new_v4();

	let original = db
		.upsert("note", "note-1", &json!({ "title": "Photo" }))
		.await
		.expect("upsert");
	let copy = db
		.upsert("note", "note-2", &json!({ "title": "Photo copy" }))
		.await
		.expect("upsert");
	let identity = ContentIdentity {
		sampled_hash: Some("abc123".to_string()),
		integrity_hash: None,
		size: Some(42),
		kind: None,
		kind_name: None,
	};
	db.set_content_identity(original, &identity)
		.await
		.expect("content");
	db.set_content_identity(copy, &identity)
		.await
		.expect("content");

	let tag = definition("Photos/Best", 100, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("definition");

	// One claim on the bytes reaches both copies.
	let mut on_bytes = apply(&tag, original, 110, device);
	on_bytes.content_uuid = Some(uuid_for("abc123"));
	db.append_tag_assertions(&[on_bytes]).await.expect("append");

	let mut with_tag = db.records_with_tag(tag.uuid).await.expect("read");
	with_tag.sort();
	let mut both = vec![original, copy];
	both.sort();
	assert_eq!(with_tag, both);

	let state = db.tags_for_records(&[original, copy]).await.expect("state");
	assert_eq!(state[&original][0].path, "Photos/Best");
	assert_eq!(state[&copy][0].path, "Photos/Best");

	// A record-scoped removal beats the earlier content-scoped apply for that
	// one copy and leaves the other alone.
	let mut removal = apply(&tag, copy, 120, device);
	removal.asserted = false;
	db.append_tag_assertions(&[removal]).await.expect("removal");

	assert_eq!(
		db.records_with_tag(tag.uuid).await.expect("read"),
		vec![original]
	);
}

#[tokio::test]
async fn late_binding_fills_the_content_key() {
	let fixture = Fixture::new().await;
	let db = fixture.create("src-1").await;
	let device = Uuid::new_v4();

	let original = db
		.upsert("note", "note-1", &json!({ "title": "Photo" }))
		.await
		.expect("upsert");
	let tag = definition("Photos/Best", 100, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("definition");

	// Applied during the walk, before hashing reached the file.
	db.append_tag_assertions(&[apply(&tag, original, 110, device)])
		.await
		.expect("append");

	let identity = ContentIdentity {
		sampled_hash: Some("abc123".to_string()),
		integrity_hash: None,
		size: Some(42),
		kind: None,
		kind_name: None,
	};
	db.set_content_identity(original, &identity)
		.await
		.expect("content");
	assert_eq!(db.bind_assertion_content().await.expect("bind"), 1);

	// A copy hashed afterwards inherits the claim through the content key.
	let copy = db
		.upsert("note", "note-2", &json!({ "title": "Photo copy" }))
		.await
		.expect("upsert");
	db.set_content_identity(copy, &identity)
		.await
		.expect("content");

	let mut with_tag = db.records_with_tag(tag.uuid).await.expect("read");
	with_tag.sort();
	let mut both = vec![original, copy];
	both.sort();
	assert_eq!(with_tag, both);
}

/// A tag applied while the bytes were only a guess keeps reaching every copy
/// after each copy is read in full, whether the copies confirm together or
/// one of them confirms first and the other stays a candidate.
#[tokio::test]
async fn a_content_tag_applied_before_verification_reaches_every_copy_afterwards() {
	let fixture = Fixture::new().await;
	let db = fixture.create("src-1").await;
	let device = Uuid::new_v4();

	let original = db
		.upsert("note", "note-1", &json!({ "title": "Photo" }))
		.await
		.expect("upsert");
	let copy = db
		.upsert("note", "note-2", &json!({ "title": "Photo copy" }))
		.await
		.expect("upsert");
	let guess = ContentIdentity {
		sampled_hash: Some("abc123".to_string()),
		integrity_hash: None,
		size: Some(42),
		kind: None,
		kind_name: None,
	};
	db.set_content_identity(original, &guess)
		.await
		.expect("content");
	db.set_content_identity(copy, &guess)
		.await
		.expect("content");

	let tag = definition("Photos/Best", 100, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("definition");
	let mut on_bytes = apply(&tag, original, 110, device);
	on_bytes.content_uuid = Some(uuid_for("abc123"));
	db.append_tag_assertions(&[on_bytes]).await.expect("append");

	let mut both = vec![original, copy];
	both.sort();
	async fn with_tag(db: &sd_store::db::SourceDb, tag: Uuid) -> Vec<Uuid> {
		let mut with_tag = db.records_with_tag(tag).await.expect("read");
		with_tag.sort();
		with_tag
	}

	// The original confirms; the copy is still a candidate.
	let read_in_full = ContentIdentity {
		integrity_hash: Some("full-abc".to_string()),
		..guess.clone()
	};
	db.set_content_identity(original, &read_in_full)
		.await
		.expect("confirm original");
	assert_eq!(with_tag(&db, tag.uuid).await, both);
	let state = db.tags_for_records(&[original, copy]).await.expect("state");
	assert_eq!(state[&original][0].path, "Photos/Best");
	assert_eq!(state[&copy][0].path, "Photos/Best");

	// The anchored assertion now carries the confirmed uuid, and the copy
	// confirms onto the same row.
	let key: Uuid =
		sqlx::query_scalar("SELECT content_uuid FROM tag_assertion WHERE record_uuid = ?")
			.bind(original)
			.fetch_one(db.pool())
			.await
			.expect("key");
	assert_eq!(key, uuid_for("full-abc"));
	db.set_content_identity(copy, &read_in_full)
		.await
		.expect("confirm copy");
	assert_eq!(with_tag(&db, tag.uuid).await, both);

	// A third copy that only samples alike is reached by the candidate uuid
	// the tag was applied under, through the confirmed row's candidate key.
	let third = db
		.upsert("note", "note-3", &json!({ "title": "Lookalike" }))
		.await
		.expect("upsert");
	db.set_content_identity(third, &guess)
		.await
		.expect("content");
	let state = db.tags_for_records(&[third]).await.expect("state");
	assert_eq!(state[&third][0].path, "Photos/Best");
}

#[tokio::test]
async fn assertions_survive_generation_loss_and_rebind() {
	let fixture = Fixture::new().await;
	let db = fixture.create("src-1").await;
	let device = Uuid::new_v4();

	let note = db
		.upsert("note", "note-9", &json!({ "title": "Keeper" }))
		.await
		.expect("upsert");
	let tag = definition("Keep", 100, device);
	db.upsert_tag_definitions(std::slice::from_ref(&tag))
		.await
		.expect("definition");
	let mut assertion = apply(&tag, note, 110, device);
	assertion.external_id = Some("note-9".to_string());
	db.append_tag_assertions(&[assertion])
		.await
		.expect("append");

	// The generation is lost: the record row goes, the assertion must not.
	sqlx::query("DELETE FROM record WHERE uuid = ?")
		.bind(note)
		.execute(db.pool())
		.await
		.expect("drop generation row");
	assert!(db
		.records_with_tag(tag.uuid)
		.await
		.expect("read")
		.is_empty());
	let orphaned: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tag_assertion")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(orphaned.0, 1, "the assertion row outlives its record");

	// A rebuild mints a fresh uuid; the evidence brings the claim home.
	let reborn = db
		.upsert("note", "note-9", &json!({ "title": "Keeper" }))
		.await
		.expect("re-upsert");
	assert_ne!(reborn, note, "rebuild minted a fresh record uuid");
	assert_eq!(db.rebind_tag_assertions().await.expect("rebind"), 1);
	assert_eq!(
		db.records_with_tag(tag.uuid).await.expect("read"),
		vec![reborn]
	);
}

#[tokio::test]
async fn identical_claims_collapse_across_stores() {
	let fixture = Fixture::new().await;
	let a = fixture.create("src-a").await;
	let b = fixture.create("src-b").await;
	let device_a = Uuid::new_v4();
	let device_b = Uuid::new_v4();

	// The same bytes indexed on two sources, tagged independently on each.
	// Content uuids are convergent, so these are the same claim computed
	// twice, and each store resolves it without coordination.
	let tag = definition("Shared", 100, device_a);
	let identity = ContentIdentity {
		sampled_hash: Some("same-bytes".to_string()),
		integrity_hash: None,
		size: Some(7),
		kind: None,
		kind_name: None,
	};

	for (db, device, external) in [(&a, device_a, "copy-a"), (&b, device_b, "copy-b")] {
		let record = db
			.upsert("note", external, &json!({ "title": "Twin" }))
			.await
			.expect("upsert");
		db.set_content_identity(record, &identity)
			.await
			.expect("content");
		db.upsert_tag_definitions(std::slice::from_ref(&tag))
			.await
			.expect("adopt");
		let mut assertion = apply(&tag, record, 110, device);
		assertion.content_uuid = Some(uuid_for("same-bytes"));
		db.append_tag_assertions(&[assertion])
			.await
			.expect("append");
	}

	assert_eq!(a.records_with_tag(tag.uuid).await.expect("read").len(), 1);
	assert_eq!(b.records_with_tag(tag.uuid).await.expect("read").len(), 1);
	assert_eq!(a.tag_definitions().await.expect("list")[0].path, "Shared");
	assert_eq!(b.tag_definitions().await.expect("list")[0].path, "Shared");
}
