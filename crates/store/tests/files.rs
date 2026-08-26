//! The filesystem ingest against a real store: resolution, the batch, and what
//! survives a file moving.

use sd_store::db::{OverlayEvidence, Stamp};
use sd_store::file::{
	FileKind, FileWrite, Ledger, Observation, Resolution, SubtreeRename, Watermark,
};
use sd_store::record::ContentIdentity;
use sd_store::{filesystem_schema, SourceDb, SourceManager};
use serde_json::json;
use uuid::Uuid;

struct Fixture {
	_dir: tempfile::TempDir,
	manager: SourceManager,
}

impl Fixture {
	async fn new() -> Self {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().join("sources"));
		manager
			.create("drive-1", &filesystem_schema())
			.await
			.expect("source created");
		Self { _dir: dir, manager }
	}

	async fn open(&self) -> SourceDb {
		self.manager.open("drive-1").await.expect("open")
	}
}

fn observe(path: &str, size: i64, mtime: i64, inode: Option<i64>) -> Observation {
	Observation {
		external_id: path.to_string(),
		kind: FileKind::File,
		name: path.rsplit('/').next().unwrap_or(path).to_string(),
		size,
		mtime,
		created: None,
		accessed: None,
		inode,
		mode: Some(0o644),
		extension: path.rsplit_once('.').map(|(_, e)| e.to_string()),
		is_hidden: false,
	}
}

fn observe_dir(path: &str) -> Observation {
	Observation {
		kind: FileKind::Directory,
		size: 0,
		mode: Some(0o755),
		extension: None,
		..observe(path, 0, 1_700_000_000_000, None)
	}
}

fn write(resolution: Resolution, observation: Observation) -> FileWrite {
	FileWrite {
		resolution,
		parent_uuid: None,
		observation,
	}
}

/// A write that knows where it hangs, which is what the store needs to address
/// it: a file stores no key of its own and is found through its parent.
fn write_under(ledger: &Ledger, resolution: Resolution, observation: Observation) -> FileWrite {
	let parent_uuid = observation
		.external_id
		.rsplit_once('/')
		.and_then(|(parent, _)| ledger.uuid_of(parent));
	FileWrite {
		resolution,
		parent_uuid,
		observation,
	}
}

/// Observe these paths the way a walk does: every directory on the way to a
/// file before the file itself, so the tree exists to address it through.
fn walk(ledger: &mut Ledger, paths: &[(&str, i64, Option<i64>)]) -> Vec<FileWrite> {
	let mut seen: Vec<String> = Vec::new();
	let mut writes = Vec::new();

	for (path, size, inode) in paths {
		let mut prefix = String::new();
		let components: Vec<&str> = path.split('/').collect();
		for directory in &components[..components.len() - 1] {
			if !prefix.is_empty() {
				prefix.push('/');
			}
			prefix.push_str(directory);
			if seen.iter().any(|s| s == &prefix) {
				continue;
			}
			seen.push(prefix.clone());
			let observation = observe_dir(&prefix);
			let resolution = ledger.resolve(&observation);
			writes.push(write_under(ledger, resolution, observation));
		}

		let observation = observe(path, *size, 1_700_000_000_000, *inode);
		let resolution = ledger.resolve(&observation);
		writes.push(write_under(ledger, resolution, observation));
	}

	writes
}

/// A record's path: its own if it is a directory, its parent's plus its name
/// otherwise. Files store no key, so this is the only way to ask where one is.
const PATH_OF_RECORD: &str = "\
COALESCE(own.path, parent.path || '/' || r.title, r.title)
   FROM record r
   LEFT JOIN directory_path own ON own.record_uuid = r.uuid
   LEFT JOIN directory_path parent ON parent.record_uuid = r.parent_uuid";

async fn paths(db: &SourceDb) -> Vec<String> {
	sqlx::query_scalar(&format!("SELECT {PATH_OF_RECORD} ORDER BY 1"))
		.fetch_all(db.pool())
		.await
		.expect("records")
}

async fn path_of(db: &SourceDb, uuid: Uuid) -> String {
	sqlx::query_scalar(&format!("SELECT {PATH_OF_RECORD} WHERE r.uuid = ?"))
		.bind(uuid)
		.fetch_one(db.pool())
		.await
		.expect("record")
}

#[tokio::test]
async fn a_walk_writes_records_and_facets() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	assert!(ledger.is_empty());

	let batch = walk(
		&mut ledger,
		&[
			("notes/a.txt", 100, Some(10)),
			("notes/b.txt", 101, Some(11)),
		],
	);

	// Two files and the directory they are addressed through.
	assert_eq!(
		db.apply_files(&batch, &[], &[], None).await.expect("apply"),
		3
	);

	let (title, size, mode): (String, i64, i64) = sqlx::query_as(&format!(
		"SELECT r.title, f.size, f.mode FROM record r
		 JOIN facet_file f ON f.record_uuid = r.uuid
		 JOIN directory_path parent ON parent.record_uuid = r.parent_uuid
		 WHERE parent.path = 'notes' AND r.title = 'a.txt'"
	))
	.fetch_one(db.pool())
	.await
	.expect("row");

	assert_eq!(title, "a.txt");
	assert_eq!(size, 100);
	assert_eq!(mode, 0o644);
}

#[tokio::test]
async fn an_unchanged_file_is_not_rewritten() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let observation = observe("a.txt", 100, 1_700_000_000_000, Some(10));
	let first = ledger.resolve(&observation);
	assert!(matches!(first, Resolution::Fresh(_)));
	db.apply_files(&[write(first, observation.clone())], &[], &[], None)
		.await
		.expect("apply");

	let second = ledger.resolve(&observation);
	assert_eq!(second, Resolution::Unchanged(first.uuid()));
	assert_eq!(
		db.apply_files(&[write(second, observation)], &[], &[], None)
			.await
			.expect("apply"),
		0,
		"a second walk over an untouched tree writes nothing"
	);
}

#[tokio::test]
async fn changed_bytes_keep_the_record_and_drop_the_content_identity() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let observation = observe("a.txt", 100, 1_700_000_000_000, Some(10));
	let fresh = ledger.resolve(&observation);
	db.apply_files(&[write(fresh, observation)], &[], &[], None)
		.await
		.expect("apply");

	db.set_content_identity(
		fresh.uuid(),
		&ContentIdentity {
			sampled_hash: Some("sampled-1".to_string()),
			..Default::default()
		},
	)
	.await
	.expect("hash");

	let edited = observe("a.txt", 240, 1_700_000_999_000, Some(10));
	let resolution = ledger.resolve(&edited);
	assert_eq!(resolution, Resolution::Changed(fresh.uuid()));
	db.apply_files(&[write(resolution, edited)], &[], &[], None)
		.await
		.expect("apply");

	let content_id: Option<i64> =
		sqlx::query_scalar("SELECT content_id FROM record WHERE uuid = ?")
			.bind(fresh.uuid())
			.fetch_one(db.pool())
			.await
			.expect("record");
	assert!(
		content_id.is_none(),
		"a hash that names the old bytes is worse than no hash"
	);
}

#[tokio::test]
async fn a_moved_file_carries_its_assertions_with_it() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let batch = walk(&mut ledger, &[("inbox/a.txt", 100, Some(10))]);
	let fresh = ledger.resolve(&observe("inbox/a.txt", 100, 1_700_000_000_000, Some(10)));
	db.apply_files(&batch, &[], &[], None).await.expect("apply");

	db.set_overlay(
		fresh.uuid(),
		&OverlayEvidence {
			type_: "file".to_string(),
			external_id: "inbox/a.txt".to_string(),
			content_uuid: None,
		},
		&Stamp {
			hlc: "1".to_string(),
			device_uuid: Uuid::nil(),
		},
		&json!({ "rating": 5 }),
	)
	.await
	.expect("rate it");

	// Same inode, same size, new path: the walk is seeing a file that moved
	// while nothing was watching.
	let mut destination = walk(&mut ledger, &[("archive/2026/a.txt", 100, Some(10))]);
	let moved = destination.pop().expect("the file itself");
	assert_eq!(moved.resolution, Resolution::Moved(fresh.uuid()));
	destination.push(moved);
	db.apply_files(&destination, &[], &[], None)
		.await
		.expect("apply");

	assert_eq!(path_of(&db, fresh.uuid()).await, "archive/2026/a.txt");
	assert_eq!(
		db.get_overlay(fresh.uuid()).await.expect("overlay")["rating"],
		json!(5),
		"the rating was never keyed to the path"
	);

	let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record WHERE type = 'file'")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(files, 1, "a move is one row updated, not two rows");

	// The old path is gone from the ledger, so something new arriving there
	// does not inherit the moved file's identity.
	let replacement = observe("inbox/a.txt", 100, 1_700_000_000_000, None);
	assert!(matches!(ledger.resolve(&replacement), Resolution::Fresh(_)));
}

#[tokio::test]
async fn inode_reuse_alone_does_not_rebind() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let observation = observe("a.txt", 100, 1_700_000_000_000, Some(10));
	let fresh = ledger.resolve(&observation);
	db.apply_files(&[write(fresh, observation)], &[], &[], None)
		.await
		.expect("apply");

	// The inode came back on an unrelated file. Neither size nor mtime agrees,
	// so the second factor is missing and the ledger declines.
	let stranger = observe("b.txt", 4096, 1_800_000_000_000, Some(10));
	let resolution = ledger.resolve(&stranger);
	assert!(matches!(resolution, Resolution::Fresh(_)));
	assert_ne!(resolution.uuid(), fresh.uuid());
}

#[tokio::test]
async fn a_batch_and_its_watermark_commit_together() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	db.set_cursor("walk", "start").await.expect("cursor");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let good = observe("a.txt", 100, 1_700_000_000_000, Some(10));
	let orphan = observe("b.txt", 100, 1_700_000_000_000, Some(11));

	let batch = vec![
		write(ledger.resolve(&good), good),
		FileWrite {
			resolution: ledger.resolve(&orphan),
			// A parent no record answers to. Stands in for any failure partway
			// through a batch.
			parent_uuid: Some(Uuid::now_v7()),
			observation: orphan,
		},
	];

	let result = db
		.apply_files(
			&batch,
			&[],
			&[],
			Some(Watermark {
				key: "walk",
				value: "finished",
			}),
		)
		.await;
	let err = result.expect_err("the batch fails as a unit").to_string();
	assert!(
		err.contains("FOREIGN KEY"),
		"expected the orphan parent to be rejected, got: {err}"
	);

	let records: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(records, 0, "no half-written batch");
	assert_eq!(
		db.get_cursor("walk").await.expect("cursor").as_deref(),
		Some("start"),
		"and no watermark claiming work that did not land"
	);
}

#[tokio::test]
async fn the_ledger_reloads_from_the_store() {
	let fixture = Fixture::new().await;
	let uuid = {
		let db = fixture.open().await;
		db.begin_sync().await.expect("epoch");
		let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
		let observation = observe("a.txt", 100, 1_700_000_000_000, Some(10));
		let fresh = ledger.resolve(&observation);
		db.apply_files(&[write(fresh, observation)], &[], &[], None)
			.await
			.expect("apply");
		fresh.uuid()
	};

	// A new attach, with nothing carried over in memory.
	let db = fixture.open().await;
	let mut ledger = Ledger::load(db.pool()).await.expect("reload");
	assert_eq!(ledger.len(), 1);
	assert_eq!(ledger.uuid_of("a.txt"), Some(uuid));

	let observation = observe("a.txt", 100, 1_700_000_000_000, Some(10));
	assert_eq!(ledger.resolve(&observation), Resolution::Unchanged(uuid));
}

#[tokio::test]
async fn a_removal_takes_the_facet_and_leaves_the_assertion() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let observation = observe("a.txt", 100, 1_700_000_000_000, Some(10));
	let fresh = ledger.resolve(&observation);
	db.apply_files(&[write(fresh, observation)], &[], &[], None)
		.await
		.expect("apply");

	db.set_overlay(
		fresh.uuid(),
		&OverlayEvidence {
			type_: "file".to_string(),
			external_id: "a.txt".to_string(),
			content_uuid: None,
		},
		&Stamp {
			hlc: "1".to_string(),
			device_uuid: Uuid::nil(),
		},
		&json!({ "rating": 5 }),
	)
	.await
	.expect("rate it");

	let removed = ledger.forget("a.txt").expect("bound");
	assert_eq!(removed, fresh.uuid());
	db.apply_files(&[], &[removed], &[], None)
		.await
		.expect("remove");

	let facets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM facet_file")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(facets, 0, "the facet cascades with its record");

	let assertions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record_overlay")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(
		assertions, 1,
		"the assertion outlives the generation and waits for a rebind"
	);
}

#[tokio::test]
async fn a_sweep_turns_absence_into_removals() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	let mut ledger = Ledger::default();

	let writes: Vec<FileWrite> = ["a.txt", "b.txt", "c.txt"]
		.iter()
		.map(|path| {
			let observation = observe(path, 10, 1_000, None);
			write(ledger.resolve(&observation), observation)
		})
		.collect();
	db.apply_files(&writes, &[], &[], None)
		.await
		.expect("first walk");

	// The second walk finds b.txt gone. Nothing tells the ledger so; only the
	// absence does.
	ledger.begin_sweep();
	assert!(ledger.is_sweeping());
	let writes: Vec<FileWrite> = ["a.txt", "c.txt"]
		.iter()
		.map(|path| {
			let observation = observe(path, 10, 1_000, None);
			write(ledger.resolve(&observation), observation)
		})
		.collect();
	let removals = ledger.finish_sweep(&[]);
	db.apply_files(&writes, &removals, &[], None)
		.await
		.expect("second walk");

	assert_eq!(removals.len(), 1);
	assert_eq!(ledger.len(), 2);
	assert!(ledger.uuid_of("b.txt").is_none());

	let remaining: Vec<String> = sqlx::query_scalar(&format!("SELECT {PATH_OF_RECORD} ORDER BY 1"))
		.fetch_all(db.pool())
		.await
		.expect("records");
	assert_eq!(remaining, vec!["a.txt", "c.txt"]);
}

#[tokio::test]
async fn an_interrupted_sweep_does_not_delete_the_source() {
	let mut ledger = Ledger::default();
	for path in ["a.txt", "b.txt"] {
		let observation = observe(path, 10, 1_000, None);
		ledger.resolve(&observation);
	}

	// A walk starts, sees one file, and dies. The next walk starts its own
	// sweep, which discards the first rather than inheriting its verdict.
	ledger.begin_sweep();
	let observation = observe("a.txt", 10, 1_000, None);
	ledger.resolve(&observation);
	ledger.begin_sweep();

	let observation = observe("a.txt", 10, 1_000, None);
	ledger.resolve(&observation);
	let observation = observe("b.txt", 10, 1_000, None);
	ledger.resolve(&observation);

	assert!(ledger.finish_sweep(&[]).is_empty());
	assert_eq!(ledger.len(), 2);
}

#[tokio::test]
async fn finishing_without_a_sweep_removes_nothing() {
	let mut ledger = Ledger::default();
	let observation = observe("a.txt", 10, 1_000, None);
	ledger.resolve(&observation);

	assert!(ledger.finish_sweep(&[]).is_empty());
	assert_eq!(ledger.len(), 1);
}

#[tokio::test]
async fn a_child_ahead_of_its_parent_still_commits() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	let mut ledger = Ledger::default();

	let mut dir = observe("docs", 0, 1_000, None);
	dir.kind = FileKind::Directory;
	let dir_write = write(ledger.resolve(&dir), dir);

	let child = observe("docs/a.txt", 10, 1_000, None);
	let child_write = FileWrite {
		resolution: ledger.resolve(&child),
		parent_uuid: Some(dir_write.uuid()),
		observation: child,
	};

	// The batch arrives child first, which is what a coalesced watcher burst
	// looks like.
	db.apply_files(&[child_write.clone(), dir_write.clone()], &[], &[], None)
		.await
		.expect("batch commits regardless of arrival order");

	let parent: Option<Uuid> = sqlx::query_scalar("SELECT parent_uuid FROM record WHERE uuid = ?")
		.bind(child_write.uuid())
		.fetch_one(db.pool())
		.await
		.expect("child");
	assert_eq!(parent, Some(dir_write.uuid()));
}

#[tokio::test]
async fn a_subtree_the_walk_could_not_read_survives_the_sweep() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	let mut ledger = Ledger::default();

	let writes = walk(
		&mut ledger,
		&[
			("open/a.txt", 10, None),
			("locked/b.txt", 10, None),
			("locked/deep/c.txt", 10, None),
			("gone.txt", 10, None),
		],
	);
	db.apply_files(&writes, &[], &[], None)
		.await
		.expect("first walk");

	// The next walk cannot open `locked`, so it reports nothing under it. That
	// is a walk that did not look, not a subtree that went away.
	ledger.begin_sweep();
	let second = walk(&mut ledger, &[("open/a.txt", 10, None)]);
	let removals = ledger.finish_sweep(&["locked".to_string()]);
	db.apply_files(&second, &removals, &[], None)
		.await
		.expect("second walk");

	let remaining: Vec<String> = sqlx::query_scalar(&format!("SELECT {PATH_OF_RECORD} ORDER BY 1"))
		.fetch_all(db.pool())
		.await
		.expect("records");
	assert_eq!(
		remaining,
		vec![
			"locked",
			"locked/b.txt",
			"locked/deep",
			"locked/deep/c.txt",
			"open",
			"open/a.txt"
		]
	);

	// The exempted bindings are still bound, so the next walk that can read
	// them resolves them as unchanged rather than rediscovering them.
	assert_eq!(ledger.len(), 6);
	assert!(ledger.uuid_of("locked/deep/c.txt").is_some());
}

#[tokio::test]
async fn an_exemption_matches_on_path_segments() {
	let mut ledger = Ledger::default();
	for path in ["locked/a.txt", "locked-elsewhere/b.txt"] {
		let observation = observe(path, 10, 1_000, None);
		ledger.resolve(&observation);
	}

	ledger.begin_sweep();
	let removals = ledger.finish_sweep(&["locked".to_string()]);

	// A shared prefix is not containment: `locked-elsewhere` was reachable and
	// the walk found nothing in it.
	assert_eq!(removals.len(), 1);
	assert!(ledger.uuid_of("locked/a.txt").is_some());
	assert!(ledger.uuid_of("locked-elsewhere/b.txt").is_none());
}

/// A file stores no key, so a path has to be answered by walking two indexes:
/// the directory's own row, then the name beneath it.
#[tokio::test]
async fn a_path_resolves_through_its_directory() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");
	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");

	let writes = walk(
		&mut ledger,
		&[("a/b/c/deep.txt", 10, Some(1)), ("top.txt", 20, Some(2))],
	);
	db.apply_files(&writes, &[], &[], None)
		.await
		.expect("apply");

	for path in ["a", "a/b", "a/b/c"] {
		let found = db.resolve_path(path).await.expect("query");
		assert_eq!(found, ledger.uuid_of(path), "directory {path}");
	}
	for path in ["a/b/c/deep.txt", "top.txt"] {
		let found = db.resolve_path(path).await.expect("query");
		assert_eq!(found, ledger.uuid_of(path), "file {path}");
	}

	assert_eq!(
		db.resolve_path("a/b/missing.txt").await.expect("query"),
		None
	);

	// Only directories carry a key of their own; a file is found through one.
	let keyed: Vec<Option<String>> =
		sqlx::query_scalar("SELECT external_id FROM record WHERE type = 'file'")
			.fetch_all(db.pool())
			.await
			.expect("files");
	assert!(keyed.iter().all(Option::is_none));
}

/// Renaming a directory re-addresses everything under it without touching a
/// single record inside it. That is the point of storing the path once.
#[tokio::test]
async fn renaming_a_directory_re_addresses_its_subtree() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");
	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");

	let writes = walk(
		&mut ledger,
		&[
			("photos/2026/a.jpg", 10, Some(1)),
			("photos/b.jpg", 20, Some(2)),
		],
	);
	db.apply_files(&writes, &[], &[], None)
		.await
		.expect("apply");
	let deep = ledger.uuid_of("photos/2026/a.jpg").expect("bound");

	let renamed = observe_dir("archive");
	let resolution = ledger.rebind("photos", &renamed).expect("known directory");
	ledger.rename_tree("photos", "archive");
	db.apply_files(
		&[write_under(&ledger, resolution, renamed)],
		&[],
		&[SubtreeRename {
			from: "photos".to_string(),
			to: "archive".to_string(),
		}],
		None,
	)
	.await
	.expect("rename");

	assert_eq!(
		paths(&db).await,
		vec![
			"archive",
			"archive/2026",
			"archive/2026/a.jpg",
			"archive/b.jpg"
		]
	);
	assert_eq!(
		db.resolve_path("archive/2026/a.jpg").await.expect("query"),
		Some(deep),
		"the file kept its identity through its parent moving"
	);
	assert_eq!(ledger.uuid_of("archive/2026/a.jpg"), Some(deep));
	assert_eq!(ledger.uuid_of("photos/2026/a.jpg"), None);
}

/// The ledger holds full paths in memory and only directories store one, so
/// reloading has to rebuild every file's path from its parent's.
#[tokio::test]
async fn a_reloaded_ledger_rebuilds_the_paths_it_was_not_given() {
	let fixture = Fixture::new().await;
	let db = fixture.open().await;
	db.begin_sync().await.expect("epoch");

	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let writes = walk(
		&mut ledger,
		&[("a/b/deep.txt", 10, Some(1)), ("top.txt", 20, Some(2))],
	);
	db.apply_files(&writes, &[], &[], None)
		.await
		.expect("apply");

	let reloaded = Ledger::load(db.pool()).await.expect("reload");
	assert_eq!(reloaded.len(), ledger.len());
	for path in ["a", "a/b", "a/b/deep.txt", "top.txt"] {
		assert_eq!(
			reloaded.uuid_of(path),
			ledger.uuid_of(path),
			"{path} came back at the same address"
		);
	}
}

/// An index built before a record could be addressed by its parent has to be
/// discarded rather than layered over.
///
/// `RECORD_SCHEMA` is `IF NOT EXISTS` throughout, so the old `external_id NOT
/// NULL` column would survive and `directory_path` would never appear, and the
/// first walk into it would fail on every file it tried to write without a key.
/// Assertions are the one thing a walk cannot rebuild, so they stay.
#[tokio::test]
async fn an_index_that_predates_parent_addressing_is_rebuilt() {
	let dir = tempfile::tempdir().expect("tempdir");
	let manager = SourceManager::new(dir.path().join("sources"));
	manager
		.create("drive-1", &filesystem_schema())
		.await
		.expect("source created");

	// Put the store back into the shape it had before, records and all.
	{
		let db = manager.open("drive-1").await.expect("open");
		sqlx::raw_sql(
			"DROP TABLE directory_path;
			 DROP TABLE record;
			 CREATE TABLE record (
			     uuid BLOB PRIMARY KEY,
			     external_id TEXT NOT NULL,
			     type TEXT NOT NULL,
			     title TEXT,
			     created_at INTEGER,
			     modified_at INTEGER,
			     parent_uuid BLOB,
			     content_id INTEGER,
			     version TEXT,
			     scan_epoch INTEGER,
			     indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
			     UNIQUE (type, external_id)
			 );
			 INSERT INTO record (uuid, external_id, type, title)
			 VALUES (x'00000000000000000000000000000001', 'notes/a.txt', 'file', 'a.txt');",
		)
		.execute(db.pool())
		.await
		.expect("old shape");

		db.set_overlay(
			Uuid::nil(),
			&OverlayEvidence {
				type_: "file".to_string(),
				external_id: "notes/a.txt".to_string(),
				content_uuid: None,
			},
			&Stamp {
				hlc: "1".to_string(),
				device_uuid: Uuid::nil(),
			},
			&json!({ "rating": 5 }),
		)
		.await
		.expect("rate it");
	}

	let db = manager.open("drive-1").await.expect("reopen");

	let keyless: i64 = sqlx::query_scalar(
		"SELECT COUNT(*) FROM pragma_table_info('record')
		  WHERE name = 'external_id' AND \"notnull\" = 0",
	)
	.fetch_one(db.pool())
	.await
	.expect("shape");
	assert_eq!(keyless, 1, "external_id is nullable again");

	assert!(paths(&db).await.is_empty(), "the generation was dropped");

	let assertions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM record_overlay")
		.fetch_one(db.pool())
		.await
		.expect("count");
	assert_eq!(assertions, 1, "assertions outlive the generation");

	// And it takes a walk, which is what the discard is for.
	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");
	let writes = walk(&mut ledger, &[("notes/a.txt", 10, Some(1))]);
	db.apply_files(&writes, &[], &[], None).await.expect("walk");
	assert_eq!(paths(&db).await, vec!["notes", "notes/a.txt"]);

	assert_eq!(db.rebind_overlays().await.expect("rebind"), 1);
}
