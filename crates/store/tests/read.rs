//! The read-only path against a real store: a cold SQLite file answering
//! listings, lookups and folded name search with no writer and no arena.

use std::collections::HashSet;

use sd_store::file::{FileKind, FileWrite, Ledger, Observation};
use sd_store::record::ContentIdentity;
use sd_store::{filesystem_schema, read, uuid_for, SourceManager};

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
}

fn observe(path: &str, kind: FileKind, size: i64, hidden: bool) -> Observation {
	Observation {
		external_id: path.to_string(),
		kind,
		name: path.rsplit('/').next().unwrap_or(path).to_string(),
		size,
		mtime: 1_700_000_000_000,
		created: Some(1_600_000_000_000),
		accessed: None,
		inode: None,
		mode: Some(0o644),
		uid: None,
		gid: None,
		link_target: None,
		extension: path.rsplit_once('.').map(|(_, e)| e.to_string()),
		is_hidden: hidden,
		identity: None,
	}
}

/// Observe a tree the way a walk does: every directory before its children,
/// so each write knows the parent it hangs from.
async fn populate(fixture: &Fixture, paths: &[(&str, FileKind, bool)]) {
	let db = fixture.manager.open("drive-1").await.expect("open");
	db.begin_sync().await.expect("epoch");
	let mut ledger = Ledger::load(db.pool()).await.expect("ledger");

	let mut writes = Vec::new();
	for (path, kind, hidden) in paths {
		let observation = observe(path, *kind, 42, *hidden);
		let resolution = ledger.resolve(&observation);
		let parent_uuid = path
			.rsplit_once('/')
			.and_then(|(parent, _)| ledger.uuid_of(parent));
		writes.push(FileWrite {
			resolution,
			parent_uuid,
			observation,
		});
	}
	db.apply_files(&writes, &[], &[], None)
		.await
		.expect("apply");
}

#[tokio::test]
async fn a_cold_store_answers_listings_and_lookups() {
	let fixture = Fixture::new().await;
	populate(
		&fixture,
		&[
			("notes", FileKind::Directory, false),
			("notes/a.txt", FileKind::File, false),
			("notes/b.txt", FileKind::File, false),
			("notes/.secret", FileKind::File, true),
			("top.png", FileKind::File, false),
		],
	)
	.await;

	let db = fixture
		.manager
		.open_read_only("drive-1")
		.await
		.expect("read-only open");

	let root = read::children_of(db.pool(), None, false)
		.await
		.expect("root listing");
	let names: Vec<&str> = root.iter().map(|e| e.name.as_str()).collect();
	assert_eq!(names, vec!["notes", "top.png"]);
	assert_eq!(root[0].kind, FileKind::Directory);
	assert_eq!(root[0].relative_path, "notes");
	assert_eq!(root[1].relative_path, "top.png");

	let notes = read::entry_by_path(db.pool(), "notes")
		.await
		.expect("lookup")
		.expect("notes exists");
	let children = read::children_of(db.pool(), Some(notes.uuid), false)
		.await
		.expect("children");
	assert_eq!(
		children.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
		vec!["a.txt", "b.txt"],
		"the hidden file stays behind its flag"
	);
	assert_eq!(children[0].relative_path, "notes/a.txt");
	assert_eq!(children[0].size, Some(42));
	assert_eq!(children[0].mtime_ms, Some(1_700_000_000_000));

	let with_hidden = read::children_of(db.pool(), Some(notes.uuid), true)
		.await
		.expect("children with hidden");
	assert_eq!(with_hidden.len(), 3);

	let file = read::entry_by_path(db.pool(), "notes/b.txt")
		.await
		.expect("lookup")
		.expect("file exists");
	assert_eq!(file.kind, FileKind::File);
	assert_eq!(file.extension.as_deref(), Some("txt"));
	assert_eq!(
		read::entry_by_uuid(db.pool(), file.uuid)
			.await
			.expect("by uuid")
			.expect("same record")
			.relative_path,
		"notes/b.txt"
	);
}

#[tokio::test]
async fn search_folds_unicode_the_way_the_arena_does() {
	let fixture = Fixture::new().await;
	populate(
		&fixture,
		&[
			("ÉLITE DANGEREUX.mp4", FileKind::File, false),
			("elsewhere.txt", FileKind::File, false),
		],
	)
	.await;

	let db = fixture
		.manager
		.open_read_only("drive-1")
		.await
		.expect("read-only open");

	let hits = read::search_titles(db.pool(), "élite dangereux", 100)
		.await
		.expect("search");
	assert_eq!(hits.total, 1, "a non-ASCII case pair still matches");
	assert_eq!(hits.entries[0].name, "ÉLITE DANGEREUX.mp4");

	let misses = read::search_titles(db.pool(), "elite", 100)
		.await
		.expect("search");
	assert_eq!(
		misses.total, 0,
		"an unaccented needle does not match an accented title, exactly as the arena behaves"
	);
}

#[tokio::test]
async fn search_reports_totals_past_its_cap() {
	let fixture = Fixture::new().await;
	populate(
		&fixture,
		&[
			("clip-1.mov", FileKind::File, false),
			("clip-2.mov", FileKind::File, false),
			("clip-3.mov", FileKind::File, false),
			("clip-4.mov", FileKind::File, false),
			("other.txt", FileKind::File, false),
		],
	)
	.await;

	let db = fixture
		.manager
		.open_read_only("drive-1")
		.await
		.expect("read-only open");

	let hits = read::search_titles(db.pool(), "clip", 2)
		.await
		.expect("search");
	assert_eq!(hits.entries.len(), 2);
	assert_eq!(hits.total, 4, "the total keeps counting past the cap");
	assert!(hits.truncated);
}

#[tokio::test]
async fn a_read_only_handle_cannot_write() {
	let fixture = Fixture::new().await;
	populate(&fixture, &[("kept.txt", FileKind::File, false)]).await;

	let db = fixture
		.manager
		.open_read_only("drive-1")
		.await
		.expect("read-only open");

	let refused = sqlx::query("INSERT INTO _sync_state (key, value) VALUES ('probe', 'x')")
		.execute(db.pool())
		.await;
	assert!(refused.is_err(), "the pool must refuse writes");

	assert!(
		fixture.manager.open_read_only("missing").await.is_err(),
		"a store that does not exist is not silently created"
	);
}

/// Files beneath a directory come out in path order, root-level files first,
/// and a scope takes its own subtree and not a sibling sharing its prefix.
/// Paged on the last row's directory and name, any page size gives the order
/// one read does.
#[tokio::test]
async fn files_beneath_a_directory_page_in_path_order() {
	let fixture = Fixture::new().await;
	populate(
		&fixture,
		&[
			("2019", FileKind::Directory, false),
			("2019/trip", FileKind::Directory, false),
			("2019/trip/b.jpg", FileKind::File, false),
			("2019/trip/a.MOV", FileKind::File, false),
			("2019/cover.jpg", FileKind::File, false),
			("2019/notes.txt", FileKind::File, false),
			("2019/.hidden.jpg", FileKind::File, true),
			("2019-extra", FileKind::Directory, false),
			("2019-extra/c.jpg", FileKind::File, false),
			("top.jpg", FileKind::File, false),
		],
	)
	.await;
	let db = fixture
		.manager
		.open_read_only("drive-1")
		.await
		.expect("read-only open");
	let media = vec!["jpg".to_string(), "mov".to_string()];
	let paths = |entries: &[read::FsEntry]| {
		entries
			.iter()
			.map(|entry| entry.relative_path.clone())
			.collect::<Vec<_>>()
	};

	// Byte order: "2019-extra" sorts before "2019/trip", since '-' is below '/'.
	let whole = [
		"top.jpg",
		"2019/cover.jpg",
		"2019-extra/c.jpg",
		"2019/trip/a.MOV",
		"2019/trip/b.jpg",
	];
	let everything =
		read::files_beneath(db.pool(), "", read::Start::First, Some(&media), false, 100)
			.await
			.expect("whole source");
	assert_eq!(paths(&everything), whole);

	let scoped = read::files_beneath(
		db.pool(),
		"2019",
		read::Start::First,
		Some(&media),
		false,
		100,
	)
	.await
	.expect("one directory");
	assert_eq!(
		paths(&scoped),
		["2019/cover.jpg", "2019/trip/a.MOV", "2019/trip/b.jpg"]
	);

	let with_hidden = read::files_beneath(db.pool(), "2019", read::Start::First, None, true, 100)
		.await
		.expect("every file");
	assert_eq!(
		paths(&with_hidden),
		[
			"2019/.hidden.jpg",
			"2019/cover.jpg",
			"2019/notes.txt",
			"2019/trip/a.MOV",
			"2019/trip/b.jpg"
		]
	);

	for limit in 1..=3 {
		let mut paged = Vec::new();
		let mut after: Option<(String, String)> = None;
		loop {
			let page = read::files_beneath(
				db.pool(),
				"",
				after
					.as_ref()
					.map_or(read::Start::First, |(directory, name)| read::Start::After {
						directory,
						name,
					}),
				Some(&media),
				false,
				limit,
			)
			.await
			.expect("page");
			paged.extend(paths(&page));
			let Some(last) = page.last().filter(|_| page.len() == limit) else {
				break;
			};
			let directory = last
				.relative_path
				.rsplit_once('/')
				.map_or("", |(directory, _)| directory);
			after = Some((directory.to_string(), last.name.clone()));
		}
		assert_eq!(paged, whole, "pages of {limit}");
	}

	// Past the root-level files, a read starts at the first directory.
	let directories = read::files_beneath(
		db.pool(),
		"",
		read::Start::Directories,
		Some(&media),
		false,
		100,
	)
	.await
	.expect("from the first directory");
	assert_eq!(paths(&directories), &whole[1..]);
}

/// A content lookup finds the bytes a scope holds wherever beneath it they
/// sit, and nothing outside it, a sibling sharing its prefix included.
#[tokio::test]
async fn contents_beneath_a_directory_answer_by_scope() {
	let fixture = Fixture::new().await;
	populate(
		&fixture,
		&[
			("2019", FileKind::Directory, false),
			("2019/trip", FileKind::Directory, false),
			("2019/trip/a.jpg", FileKind::File, false),
			("2019/b.jpg", FileKind::File, false),
			("2019-extra", FileKind::Directory, false),
			("2019-extra/c.jpg", FileKind::File, false),
			("top.jpg", FileKind::File, false),
		],
	)
	.await;
	let db = fixture.manager.open("drive-1").await.expect("open");
	for (path, hash) in [
		("2019/trip/a.jpg", "a"),
		("2019/b.jpg", "b"),
		("2019-extra/c.jpg", "c"),
		("top.jpg", "top"),
	] {
		let record = read::entry_by_path(db.pool(), path)
			.await
			.expect("lookup")
			.expect("file exists")
			.uuid;
		db.set_content_identity(
			record,
			&ContentIdentity {
				sampled_hash: Some(hash.to_string()),
				size: Some(42),
				..Default::default()
			},
		)
		.await
		.expect("identity");
	}

	let asked = ["a", "b", "c", "top", "elsewhere"].map(uuid_for);
	let in_2019 = read::contents_beneath(db.pool(), &asked, "2019")
		.await
		.expect("one directory");
	assert_eq!(in_2019, HashSet::from([uuid_for("a"), uuid_for("b")]));

	let everywhere = read::contents_beneath(db.pool(), &asked, "")
		.await
		.expect("whole source");
	assert_eq!(
		everywhere,
		HashSet::from(["a", "b", "c", "top"].map(uuid_for))
	);
}
