//! The content text index against a real record store: what a pass has to
//! read, what a search returns, and how both follow the records.

use sd_store::file::{FileKind, FileWrite, Ledger, Observation};
use sd_store::{
	filesystem_schema, match_expression, ContentTextIndex, DescriptionDoc, DescriptionSpan,
	PendingText, SourceDb, SourceManager, TextDoc, TextSearch, TranscriptDoc, TranscriptSpan,
	CAPTION_KIND, SCENE_KIND,
};
use uuid::Uuid;

const EXTENSIONS: &[&str] = &["md", "txt"];

struct Fixture {
	dir: tempfile::TempDir,
	db: SourceDb,
	ledger: Ledger,
}

impl Fixture {
	async fn new() -> Self {
		let dir = tempfile::tempdir().expect("tempdir");
		let manager = SourceManager::new(dir.path().join("sources"));
		let db = manager
			.ensure("drive-1", &filesystem_schema())
			.await
			.expect("store");
		db.begin_sync().await.expect("epoch");
		let ledger = Ledger::load(db.pool()).await.expect("ledger");
		Self { dir, db, ledger }
	}

	async fn index(&self) -> ContentTextIndex {
		ContentTextIndex::open(&self.dir.path().join("sources").join("drive-1"))
			.await
			.expect("text index")
	}

	/// Write one observation per path, parents first, the way a walk does.
	async fn observe(&mut self, files: &[(&str, i64)]) -> Vec<Uuid> {
		let mut writes = Vec::new();
		let mut uuids = Vec::new();
		for (path, size) in files {
			let parts: Vec<&str> = path.split('/').collect();
			for depth in 1..parts.len() {
				let dir = parts[..depth].join("/");
				let observation = Observation {
					kind: FileKind::Directory,
					size: 0,
					extension: None,
					..observation(&dir, 0)
				};
				writes.push(self.write(observation));
			}
			let write = self.write(observation(path, *size));
			uuids.push(write.resolution.uuid());
			writes.push(write);
		}
		self.db
			.apply_files(&writes, &[], &[], None)
			.await
			.expect("apply");
		uuids
	}

	fn write(&mut self, observation: Observation) -> FileWrite {
		let resolution = self.ledger.resolve(&observation);
		let parent_uuid = observation
			.external_id
			.rsplit_once('/')
			.and_then(|(parent, _)| self.ledger.uuid_of(parent));
		FileWrite {
			resolution,
			parent_uuid,
			observation,
		}
	}
}

fn observation(path: &str, size: i64) -> Observation {
	Observation {
		external_id: path.to_string(),
		kind: FileKind::File,
		name: path.rsplit('/').next().unwrap_or(path).to_string(),
		size,
		mtime: 1_700_000_000_000 + size,
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

fn doc(pending: &PendingText, kind: &str, text: &str) -> TextDoc {
	TextDoc {
		record_uuid: pending.record_uuid,
		size: pending.size,
		mtime: pending.mtime,
		extension: pending.extension.clone(),
		kind: kind.to_string(),
		text: text.to_string(),
		truncated: false,
		skipped: None,
	}
}

fn search(query: &str) -> TextSearch {
	TextSearch {
		expression: match_expression(query),
		limit: 50,
		snippet_tokens: 8,
		..TextSearch::default()
	}
}

async fn hit_paths(index: &ContentTextIndex, search: &TextSearch) -> Vec<String> {
	index
		.search(search)
		.await
		.expect("search")
		.into_iter()
		.map(|hit| hit.external_id)
		.collect()
}

#[tokio::test]
async fn pending_files_are_read_once_and_searchable_by_path_and_kind() {
	let mut f = Fixture::new().await;
	f.observe(&[
		("notes/a.md", 10),
		("notes/deep/b.txt", 11),
		("top.txt", 12),
		("photo.jpg", 13),
	])
	.await;
	let index = f.index().await;

	let pending = index.pending(EXTENSIONS, 0, 100).await.expect("pending");
	let mut names: Vec<&str> = pending.iter().map(|p| p.external_id.as_str()).collect();
	names.sort();
	assert_eq!(names, ["notes/a.md", "notes/deep/b.txt", "top.txt"]);
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 3);
	assert_eq!(
		index
			.pending_count_under(EXTENSIONS, "notes")
			.await
			.unwrap(),
		2
	);
	assert_eq!(
		index
			.pending_count_under(EXTENSIONS, "notes/deep")
			.await
			.unwrap(),
		1
	);
	assert_eq!(
		index
			.pending_count_under(EXTENSIONS, "top.txt")
			.await
			.unwrap(),
		1
	);
	assert_eq!(
		index.pending_count_under(EXTENSIONS, "note").await.unwrap(),
		0
	);

	let by_name = |name: &str| pending.iter().find(|p| p.external_id == name).unwrap();
	index
		.write(&[
			doc(
				by_name("notes/a.md"),
				"markdown",
				"the quick brown fox jumps",
			),
			doc(
				by_name("notes/deep/b.txt"),
				"text",
				"a fox in the deep folder fox fox",
			),
			doc(by_name("top.txt"), "text", "nothing to see"),
		])
		.await
		.expect("write");
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 0);

	// Ranked by BM25: the file that says fox three times comes first.
	assert_eq!(
		hit_paths(&index, &search("fox")).await,
		["notes/deep/b.txt", "notes/a.md"]
	);
	let hits = index.search(&search("quick")).await.unwrap();
	assert_eq!(hits.len(), 1);
	assert!(hits[0].score > 0.0);
	assert!(hits[0]
		.snippet
		.iter()
		.any(|part| part.matched && part.text == "quick"));

	let under_deep = TextSearch {
		under: "notes/deep".into(),
		..search("fox")
	};
	assert_eq!(hit_paths(&index, &under_deep).await, ["notes/deep/b.txt"]);
	let one_file = TextSearch {
		under: "notes/a.md".into(),
		..search("fox")
	};
	assert_eq!(hit_paths(&index, &one_file).await, ["notes/a.md"]);
	let sibling_prefix = TextSearch {
		under: "note".into(),
		..search("fox")
	};
	assert!(hit_paths(&index, &sibling_prefix).await.is_empty());
	let markdown_only = TextSearch {
		kinds: vec!["markdown".into()],
		..search("fox")
	};
	assert_eq!(hit_paths(&index, &markdown_only).await, ["notes/a.md"]);
	assert_eq!(
		hit_paths(&index, &search("\"in the deep\"")).await,
		["notes/deep/b.txt"]
	);
	assert!(hit_paths(&index, &search("\"deep the in\""))
		.await
		.is_empty());
	let page = TextSearch {
		limit: 1,
		offset: 1,
		..search("fox")
	};
	assert_eq!(hit_paths(&index, &page).await, ["notes/a.md"]);
}

#[tokio::test]
async fn a_changed_file_leaves_results_until_it_is_read_again() {
	let mut f = Fixture::new().await;
	f.observe(&[("a.md", 10)]).await;
	let index = f.index().await;
	let pending = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	index
		.write(&[doc(&pending[0], "markdown", "old words")])
		.await
		.unwrap();
	assert_eq!(hit_paths(&index, &search("old")).await, ["a.md"]);

	f.observe(&[("a.md", 20)]).await;
	assert!(
		hit_paths(&index, &search("old")).await.is_empty(),
		"text read from other bytes is never returned"
	);
	let pending = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	assert_eq!(pending.len(), 1);
	assert_eq!(pending[0].size, Some(20));
	index
		.write(&[doc(&pending[0], "markdown", "new words")])
		.await
		.unwrap();
	assert!(hit_paths(&index, &search("old")).await.is_empty());
	assert_eq!(hit_paths(&index, &search("new")).await, ["a.md"]);
	assert_eq!(index.counts().await.unwrap().files, 1);
}

#[tokio::test]
async fn an_unreadable_file_waits_for_new_bytes_instead_of_every_pass() {
	let mut f = Fixture::new().await;
	f.observe(&[("locked.md", 10)]).await;
	let index = f.index().await;
	let pending = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	index
		.write(&[TextDoc {
			skipped: Some("unreadable: permission denied".into()),
			..doc(&pending[0], "markdown", "")
		}])
		.await
		.unwrap();
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 0);

	f.observe(&[("locked.md", 20)]).await;
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 1);
}

#[tokio::test]
async fn a_deleted_file_is_unreachable_and_pruned() {
	let mut f = Fixture::new().await;
	let uuids = f.observe(&[("a.md", 10), ("b.md", 11)]).await;
	let index = f.index().await;
	let pending = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	let docs: Vec<_> = pending
		.iter()
		.map(|p| doc(p, "markdown", "shared words"))
		.collect();
	index.write(&docs).await.unwrap();

	f.db.apply_files(&[], &[uuids[0]], &[], None).await.unwrap();
	assert_eq!(hit_paths(&index, &search("shared")).await, ["b.md"]);
	assert_eq!(index.prune(EXTENSIONS).await.unwrap(), 1);
	assert_eq!(index.counts().await.unwrap().files, 1);
	assert_eq!(index.prune(EXTENSIONS).await.unwrap(), 0);

	// An extension no longer indexed is pruned too.
	assert_eq!(index.prune(&["txt"]).await.unwrap(), 1);
	assert_eq!(index.counts().await.unwrap().files, 0);
}

#[tokio::test]
async fn clearing_the_index_leaves_the_records_and_requeues_every_file() {
	let mut f = Fixture::new().await;
	f.observe(&[("a.md", 10), ("b.txt", 11)]).await;
	let index = f.index().await;
	let pending = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	let docs: Vec<_> = pending.iter().map(|p| doc(p, "text", "words")).collect();
	index.write(&docs).await.unwrap();
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 0);

	index.clear().await.expect("clear");
	assert_eq!(index.counts().await.unwrap().files, 0);
	assert!(hit_paths(&index, &search("words")).await.is_empty());
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 2);
	assert_eq!(f.db.count_all().await.unwrap(), 2, "records are untouched");

	// Reopening finds the same, empty, file.
	index.close().await;
	let index = f.index().await;
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 2);
}

#[tokio::test]
async fn a_rename_to_another_extension_is_read_again() {
	let mut f = Fixture::new().await;
	let uuids = f.observe(&[("report.md", 10)]).await;
	let index = f.index().await;
	let pending = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	index
		.write(&[doc(&pending[0], "markdown", "renamed words")])
		.await
		.unwrap();
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 0);

	// The bytes, size and mtime are unchanged; only the extension moved.
	let rename = |extension: &'static str| {
		sqlx::query("UPDATE facet_file SET extension = ? WHERE record_uuid = ?")
			.bind(extension)
			.bind(uuids[0])
			.execute(f.db.pool())
	};
	rename("txt").await.unwrap();
	assert!(
		hit_paths(&index, &search("renamed")).await.is_empty(),
		"text read under another extractor is not returned"
	);
	let pending = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	assert_eq!(pending.len(), 1);
	assert_eq!(pending[0].extension, "txt");

	// To an extension nothing reads: unreachable at once, pruned later.
	rename("bin").await.unwrap();
	assert!(hit_paths(&index, &search("renamed")).await.is_empty());
	assert_eq!(index.pending_count(EXTENSIONS).await.unwrap(), 0);
	assert_eq!(index.prune(EXTENSIONS).await.unwrap(), 1);
}

#[tokio::test]
async fn clearing_raises_the_epoch() {
	let f = Fixture::new().await;
	let index = f.index().await;
	assert_eq!(index.epoch().await.unwrap(), 0);
	index.clear().await.unwrap();
	index.clear().await.unwrap();
	assert_eq!(index.epoch().await.unwrap(), 2);
}

#[tokio::test]
async fn a_search_with_nothing_to_match_returns_nothing() {
	let mut f = Fixture::new().await;
	f.observe(&[("a.md", 10)]).await;
	let index = f.index().await;
	assert!(index.search(&search("   ")).await.unwrap().is_empty());
	assert!(index.search(&search("anything")).await.unwrap().is_empty());
}

const MEDIA: &[&str] = &["mp4", "m4a"];

fn transcript(pending: &sd_store::PendingTranscript, spans: &[(i64, i64, &str)]) -> TranscriptDoc {
	TranscriptDoc {
		record_uuid: pending.record_uuid,
		size: pending.size,
		mtime: pending.mtime,
		extension: pending.extension.clone(),
		language: Some("en".into()),
		model: "whisper-tiny".into(),
		duration_ms: Some(60_000),
		spans: spans
			.iter()
			.map(|(start_ms, end_ms, text)| TranscriptSpan {
				start_ms: *start_ms,
				end_ms: *end_ms,
				text: text.to_string(),
			})
			.collect(),
		skipped: None,
	}
}

#[tokio::test]
async fn a_transcript_hit_carries_the_time_it_was_said() {
	let mut f = Fixture::new().await;
	f.observe(&[
		("clips/talk.mp4", 20),
		("clips/memo.m4a", 21),
		("notes/a.md", 22),
	])
	.await;
	let index = f.index().await;

	let pending = index
		.pending_transcripts(MEDIA, "", 0, 100)
		.await
		.expect("pending");
	let mut names: Vec<&str> = pending.iter().map(|p| p.external_id.as_str()).collect();
	names.sort();
	assert_eq!(names, ["clips/memo.m4a", "clips/talk.mp4"]);
	assert_eq!(
		index
			.pending_transcripts(MEDIA, "clips/talk.mp4", 0, 100)
			.await
			.unwrap()
			.len(),
		1
	);
	let by_name = |name: &str| pending.iter().find(|p| p.external_id == name).unwrap();

	index
		.write_transcript(&transcript(
			by_name("clips/talk.mp4"),
			&[
				(0, 4_000, "Welcome to the harbour tour."),
				(41_500, 45_000, "The lighthouse was built in 1902."),
			],
		))
		.await
		.unwrap();
	let mut silent = transcript(by_name("clips/memo.m4a"), &[]);
	silent.skipped = Some("no audio track".into());
	index.write_transcript(&silent).await.unwrap();
	let notes = index.pending(EXTENSIONS, 0, 100).await.unwrap();
	index
		.write(&[doc(&notes[0], "markdown", "the lighthouse keeper's notes")])
		.await
		.unwrap();

	assert!(index
		.pending_transcripts(MEDIA, "", 0, 100)
		.await
		.unwrap()
		.is_empty());
	assert_eq!(index.transcript_count().await.unwrap(), 2);

	let hits = index
		.search(&search("\"lighthouse was built\""))
		.await
		.unwrap();
	assert_eq!(hits.len(), 1);
	assert_eq!(hits[0].external_id, "clips/talk.mp4");
	assert_eq!(hits[0].kind, "transcript");
	assert_eq!(
		(hits[0].start_ms, hits[0].end_ms),
		(Some(41_500), Some(45_000))
	);

	// Both a document and a spoken segment answer a word they share, and
	// a kind filter picks one.
	let mut kinds: Vec<String> = index
		.search(&search("lighthouse"))
		.await
		.unwrap()
		.into_iter()
		.map(|hit| hit.kind)
		.collect();
	kinds.sort();
	assert_eq!(kinds, ["markdown", "transcript"]);
	let only = |kind: &str| TextSearch {
		kinds: vec![kind.to_string()],
		..search("lighthouse")
	};
	assert_eq!(
		hit_paths(&index, &only("transcript")).await,
		["clips/talk.mp4"]
	);
	assert_eq!(hit_paths(&index, &only("markdown")).await, ["notes/a.md"]);
	let under = TextSearch {
		under: "notes".into(),
		..search("lighthouse")
	};
	assert_eq!(hit_paths(&index, &under).await, ["notes/a.md"]);

	// Rebuilding the document index keeps what was said.
	index.clear().await.unwrap();
	assert_eq!(
		hit_paths(&index, &only("transcript")).await,
		["clips/talk.mp4"]
	);

	// Forgetting a folder's transcripts queues its files again.
	assert_eq!(index.forget_transcripts("clips").await.unwrap(), 2);
	assert!(hit_paths(&index, &only("transcript")).await.is_empty());
	assert_eq!(
		index
			.pending_transcripts(MEDIA, "", 0, 100)
			.await
			.unwrap()
			.len(),
		2
	);
}

#[tokio::test]
async fn an_edited_recording_leaves_results_until_transcribed_again() {
	let mut f = Fixture::new().await;
	f.observe(&[("talk.mp4", 30)]).await;
	let index = f.index().await;
	let pending = index.pending_transcripts(MEDIA, "", 0, 10).await.unwrap();
	index
		.write_transcript(&transcript(&pending[0], &[(1_000, 2_000, "first take")]))
		.await
		.unwrap();
	assert_eq!(hit_paths(&index, &search("take")).await, ["talk.mp4"]);

	f.observe(&[("talk.mp4", 31)]).await;
	assert!(hit_paths(&index, &search("take")).await.is_empty());
	assert_eq!(
		index
			.pending_transcripts(MEDIA, "", 0, 10)
			.await
			.unwrap()
			.len(),
		1
	);
}

#[tokio::test]
async fn a_description_hit_answers_for_every_file_with_the_bytes() {
	let mut f = Fixture::new().await;
	let uuids = f
		.observe(&[
			("clips/harbour.mp4", 30),
			("backup/harbour.mp4", 30),
			("photos/pier.jpg", 31),
			("clips/other.mp4", 32),
		])
		.await;
	let identity = |hash: &str| sd_store::ContentIdentity {
		sampled_hash: Some(hash.to_string()),
		integrity_hash: None,
		size: Some(30),
		kind: None,
		kind_name: None,
	};
	f.db.set_content_identity(uuids[0], &identity("aaaa"))
		.await
		.unwrap();
	f.db.set_content_identity(uuids[1], &identity("aaaa"))
		.await
		.unwrap();
	f.db.set_content_identity(uuids[2], &identity("bbbb"))
		.await
		.unwrap();
	let video = sd_store::uuid_for("aaaa");
	let photo = sd_store::uuid_for("bbbb");
	let index = f.index().await;

	assert!(index.has_content(video).await.unwrap());
	assert!(!index.has_content(Uuid::new_v4()).await.unwrap());

	let scene = |text: &str| DescriptionDoc {
		content_uuid: video,
		kind: SCENE_KIND.into(),
		spans: vec![
			DescriptionSpan {
				start_ms: Some(0),
				end_ms: Some(4_000),
				text: "A tugboat leaves the dock at dawn".into(),
			},
			DescriptionSpan {
				start_ms: Some(4_000),
				end_ms: Some(9_500),
				text: text.into(),
			},
		],
	};
	index
		.write_descriptions(&scene("A red lighthouse on a rocky point"))
		.await
		.unwrap();
	index
		.write_descriptions(&DescriptionDoc {
			content_uuid: photo,
			kind: CAPTION_KIND.into(),
			spans: vec![DescriptionSpan {
				start_ms: None,
				end_ms: None,
				text: "Fishermen mending nets beside the lighthouse".into(),
			}],
		})
		.await
		.unwrap();

	let hits = index.search(&search("lighthouse")).await.unwrap();
	let mut found: Vec<(&str, &str, Option<i64>, Option<i64>)> = hits
		.iter()
		.map(|hit| {
			(
				hit.external_id.as_str(),
				hit.kind.as_str(),
				hit.start_ms,
				hit.end_ms,
			)
		})
		.collect();
	found.sort();
	assert_eq!(
		found,
		[
			("backup/harbour.mp4", "scene", Some(4_000), Some(9_500)),
			("clips/harbour.mp4", "scene", Some(4_000), Some(9_500)),
			("photos/pier.jpg", "caption", None, None),
		]
	);

	let only = |kind: &str| TextSearch {
		kinds: vec![kind.to_string()],
		..search("lighthouse")
	};
	assert_eq!(
		hit_paths(&index, &only("caption")).await,
		["photos/pier.jpg"]
	);
	assert!(hit_paths(&index, &only("markdown")).await.is_empty());
	let under = TextSearch {
		under: "clips".into(),
		..search("lighthouse")
	};
	assert_eq!(hit_paths(&index, &under).await, ["clips/harbour.mp4"]);

	// A rewritten sidecar replaces what the last one said.
	index
		.write_descriptions(&scene("A ferry crossing the bay"))
		.await
		.unwrap();
	assert!(hit_paths(&index, &only("scene")).await.is_empty());
	assert_eq!(
		hit_paths(
			&index,
			&TextSearch {
				kinds: vec!["scene".into()],
				..search("ferry")
			}
		)
		.await
		.len(),
		2
	);

	// Rebuilding the document index keeps the descriptions.
	index.clear().await.unwrap();
	assert_eq!(
		hit_paths(&index, &only("caption")).await,
		["photos/pier.jpg"]
	);

	// A deleted sidecar drops its rows.
	assert!(index
		.forget_descriptions(photo, CAPTION_KIND)
		.await
		.unwrap());
	assert!(!index
		.forget_descriptions(photo, CAPTION_KIND)
		.await
		.unwrap());
	assert!(hit_paths(&index, &only("caption")).await.is_empty());
}
