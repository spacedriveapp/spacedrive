//! The image facet: pending by content hash, written to every copy, and
//! re-queued when the bytes behind a record change.

use sd_store::file::{FileWrite, Ledger, Observation, Resolution};
use sd_store::record::ContentIdentity;
use sd_store::{
	count_files_needing_image_facets, files_needing_image_facets, filesystem_schema,
	set_image_facets, ImageFacet, SourceDb, SourceManager,
};
use uuid::Uuid;

const IMAGE: i64 = 1;

fn observe(path: &str, size: i64, mtime: i64) -> Observation {
	Observation {
		external_id: path.to_string(),
		kind: sd_store::FileKind::File,
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

async fn store() -> (tempfile::TempDir, SourceDb) {
	let dir = tempfile::tempdir().expect("tempdir");
	let manager = SourceManager::new(dir.path().join("sources"));
	manager
		.create("drive-1", &filesystem_schema())
		.await
		.expect("source created");
	let db = manager.open("drive-1").await.expect("open");
	db.begin_sync().await.expect("epoch");
	(dir, db)
}

async fn walk_one(db: &SourceDb, ledger: &mut Ledger, observation: Observation) -> Uuid {
	let resolution = ledger.resolve(&observation);
	let uuid = resolution.uuid();
	db.apply_files(
		&[FileWrite {
			resolution,
			parent_uuid: None,
			observation,
		}],
		&[],
		&[],
		None,
	)
	.await
	.expect("apply");
	uuid
}

async fn identify(db: &SourceDb, uuid: Uuid, hash: &str, kind: i64) {
	db.set_content_identity(
		uuid,
		&ContentIdentity {
			sampled_hash: Some(hash.to_string()),
			kind: Some(kind),
			..Default::default()
		},
	)
	.await
	.expect("identity");
}

async fn facet_of(db: &SourceDb, uuid: Uuid) -> Option<(Option<String>, Option<String>)> {
	sqlx::query_as("SELECT content_hash, date_taken FROM facet_image WHERE record_uuid = ?")
		.bind(uuid)
		.fetch_optional(db.pool())
		.await
		.expect("facet")
}

#[tokio::test]
async fn image_facets_are_written_once_per_content_hash_and_requeued_on_change() {
	let (_dir, db) = store().await;
	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");

	let a = walk_one(&db, &mut ledger, observe("a.jpg", 100, 1_000)).await;
	let b = walk_one(&db, &mut ledger, observe("b.jpg", 100, 2_000)).await;
	let c = walk_one(&db, &mut ledger, observe("c.jpg", 300, 3_000)).await;
	let text = walk_one(&db, &mut ledger, observe("d.txt", 50, 4_000)).await;

	// Nothing is pending until identification names the bytes an image.
	assert_eq!(
		count_files_needing_image_facets(db.pool(), IMAGE)
			.await
			.unwrap(),
		0
	);
	identify(&db, a, "same-bytes", IMAGE).await;
	identify(&db, b, "same-bytes", IMAGE).await;
	identify(&db, c, "other-bytes", IMAGE).await;
	identify(&db, text, "text-bytes", 7).await;
	assert_eq!(
		count_files_needing_image_facets(db.pool(), IMAGE)
			.await
			.unwrap(),
		3
	);

	let pending = files_needing_image_facets(db.pool(), IMAGE, 0, 10)
		.await
		.unwrap();
	assert_eq!(
		pending
			.iter()
			.map(|p| p.external_id.as_str())
			.collect::<Vec<_>>(),
		["a.jpg", "b.jpg", "c.jpg"]
	);
	assert_eq!(pending[0].content_hash, "same-bytes");

	// One write per hash lands on every copy of the bytes.
	let facet = ImageFacet {
		date_taken: Some("2024-03-12T10:00:00+00:00".to_string()),
		latitude: Some(35.68),
		..Default::default()
	};
	let written = set_image_facets(db.pool(), &[("same-bytes".to_string(), facet)])
		.await
		.unwrap();
	assert_eq!(written, 2);
	assert_eq!(
		facet_of(&db, a).await,
		Some((
			Some("same-bytes".to_string()),
			Some("2024-03-12T10:00:00+00:00".to_string())
		))
	);
	assert_eq!(facet_of(&db, a).await, facet_of(&db, b).await);
	assert_eq!(facet_of(&db, c).await, None);

	// An empty facet still records that the bytes were read.
	set_image_facets(
		db.pool(),
		&[("other-bytes".to_string(), ImageFacet::default())],
	)
	.await
	.unwrap();
	assert_eq!(
		count_files_needing_image_facets(db.pool(), IMAGE)
			.await
			.unwrap(),
		0
	);

	// The cursor passes over what an earlier claim already returned.
	assert!(
		files_needing_image_facets(db.pool(), IMAGE, pending[2].rowid, 10)
			.await
			.unwrap()
			.is_empty()
	);

	// New bytes under the same record: the walk drops the content id, and
	// the next identity is a hash the facet row does not name.
	let edited = observe("c.jpg", 310, 5_000);
	let resolution = ledger.resolve(&edited);
	assert_eq!(resolution, Resolution::Changed(c));
	db.apply_files(
		&[FileWrite {
			resolution,
			parent_uuid: None,
			observation: edited,
		}],
		&[],
		&[],
		None,
	)
	.await
	.expect("apply");
	assert_eq!(
		count_files_needing_image_facets(db.pool(), IMAGE)
			.await
			.unwrap(),
		0,
		"no content row, nothing to read yet"
	);
	identify(&db, c, "newer-bytes", IMAGE).await;
	let pending = files_needing_image_facets(db.pool(), IMAGE, 0, 10)
		.await
		.unwrap();
	assert_eq!(pending.len(), 1);
	assert_eq!(pending[0].uuid, c);
	assert_eq!(pending[0].content_hash, "newer-bytes");

	set_image_facets(
		db.pool(),
		&[(
			"newer-bytes".to_string(),
			ImageFacet {
				camera_make: Some("Canon".to_string()),
				..Default::default()
			},
		)],
	)
	.await
	.unwrap();
	let (hash, make): (Option<String>, Option<String>) =
		sqlx::query_as("SELECT content_hash, camera_make FROM facet_image WHERE record_uuid = ?")
			.bind(c)
			.fetch_one(db.pool())
			.await
			.unwrap();
	assert_eq!(hash.as_deref(), Some("newer-bytes"));
	assert_eq!(make.as_deref(), Some("Canon"));
	assert_eq!(
		count_files_needing_image_facets(db.pool(), IMAGE)
			.await
			.unwrap(),
		0
	);
}

/// A store created before the column existed gains it on open, and its
/// old facet rows read as pending rather than as current.
#[tokio::test]
async fn a_facet_row_without_a_hash_is_pending() {
	let (_dir, db) = store().await;
	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let a = walk_one(&db, &mut ledger, observe("a.jpg", 100, 1_000)).await;
	identify(&db, a, "bytes", IMAGE).await;
	sqlx::query("INSERT INTO facet_image (record_uuid, width) VALUES (?, 640)")
		.bind(a)
		.execute(db.pool())
		.await
		.unwrap();
	assert_eq!(
		count_files_needing_image_facets(db.pool(), IMAGE)
			.await
			.unwrap(),
		1
	);
}
