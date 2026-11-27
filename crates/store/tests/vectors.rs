//! The vector index against a real record store and text index: what an
//! embedding pass has to embed, how the scope bounds it, and how hybrid
//! ranking fuses words and meaning.

use sd_store::file::{FileKind, FileWrite, Ledger, Observation};
use sd_store::DeepScope;
use sd_store::{
	hybrid_search, ContentTextIndex, DescriptionDoc, DescriptionSpan, HybridSearch, Origin,
	PendingText, SourceDb, SourceManager, TextDoc, TextFilter, TranscriptDoc, TranscriptSpan,
	VectorIndex, VectorSearch, SCENE_KIND,
};
use uuid::Uuid;

const DIM: usize = 16;

/// A stand-in embedder: words map to concepts, so synonyms land on the same
/// axis the way a sentence embedder puts them close together.
fn embed(text: &str) -> Vec<f32> {
	let mut v = [0.0f32; DIM];
	for word in text
		.split(|c: char| !c.is_alphanumeric())
		.filter(|w| !w.is_empty())
	{
		let word = word.to_lowercase();
		let concept = match word.as_str() {
			"car" | "automobile" | "vehicle" | "sedan" => 1,
			"sea" | "ocean" | "harbour" | "bay" | "coast" => 2,
			"lighthouse" | "beacon" => 3,
			"budget" | "money" | "costs" | "finance" | "spending" => 4,
			"dog" | "puppy" | "hound" => 5,
			_ => 6 + (word.bytes().map(usize::from).sum::<usize>() % (DIM - 6)),
		};
		v[concept] += if concept < 6 { 3.0 } else { 0.2 };
	}
	let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
	v.iter().map(|x| x / norm).collect()
}

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
			.ensure("drive-1", &sd_store::filesystem_schema())
			.await
			.expect("store");
		db.begin_sync().await.expect("epoch");
		let ledger = Ledger::load(db.pool()).await.expect("ledger");
		Self { dir, db, ledger }
	}

	fn source_dir(&self) -> std::path::PathBuf {
		self.dir.path().join("sources").join("drive-1")
	}

	async fn indexes(&self) -> (ContentTextIndex, VectorIndex) {
		let text = ContentTextIndex::open(&self.source_dir())
			.await
			.expect("text index");
		let vectors = VectorIndex::open(&self.source_dir())
			.await
			.expect("vector index");
		vectors.use_model("stub", DIM).await.expect("model");
		(text, vectors)
	}

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

/// Read every pending text file with the given texts, by path.
async fn read_texts(text: &ContentTextIndex, texts: &[(&str, &str)]) {
	let pending = text
		.pending(&TextFilter::everywhere(&["md", "txt"]), 0, 100)
		.await
		.expect("pending");
	let docs: Vec<TextDoc> = pending
		.iter()
		.filter_map(|p: &PendingText| {
			let (_, body) = texts.iter().find(|(path, _)| *path == p.external_id)?;
			Some(TextDoc {
				record_uuid: p.record_uuid,
				size: p.size,
				mtime: p.mtime,
				extension: p.extension.clone(),
				kind: "markdown".into(),
				text: body.to_string(),
				truncated: false,
				skipped: None,
			})
		})
		.collect();
	text.write(&docs).await.expect("write text");
}

/// What an embedding pass does: claim, chunk, embed, write. Returns the
/// documents it embedded.
async fn embed_pass(vectors: &VectorIndex, scope: &DeepScope) -> usize {
	vectors.prune(scope).await.expect("prune");
	let mut done = 0;
	let mut cursor = (Origin::Text, 0);
	loop {
		let batch = vectors.pending(scope, cursor, 8).await.expect("pending");
		let Some(last) = batch.last() else {
			return done;
		};
		cursor = (last.origin, last.doc_id);
		for doc in &batch {
			let chunks = vectors.chunks(doc).await.expect("chunks");
			let embedded: Vec<Vec<f32>> = chunks.iter().map(|c| embed(&c.text)).collect();
			vectors.write(doc, &chunks, &embedded).await.expect("write");
			done += 1;
		}
	}
}

fn whole() -> DeepScope {
	DeepScope::whole()
}

async fn semantic_paths(vectors: &VectorIndex, query: &str, scope: &DeepScope) -> Vec<String> {
	vectors
		.search(&VectorSearch {
			vector: embed(query),
			scope: scope.clone(),
			limit: 10,
			..Default::default()
		})
		.await
		.expect("search")
		.into_iter()
		.map(|hit| hit.external_id)
		.collect()
}

#[tokio::test]
async fn documents_are_embedded_once_and_found_by_meaning() {
	let mut f = Fixture::new().await;
	f.observe(&[
		("notes/cars.md", 10),
		("notes/sea.md", 11),
		("notes/empty.md", 12),
	])
	.await;
	let (text, vectors) = f.indexes().await;
	read_texts(
		&text,
		&[
			(
				"notes/cars.md",
				"My automobile needs new tyres before winter.",
			),
			(
				"notes/sea.md",
				"We walked along the coast and watched the ocean.",
			),
			("notes/empty.md", ""),
		],
	)
	.await;

	assert_eq!(
		vectors.pending_count(&whole()).await.unwrap(),
		2,
		"empty text has nothing to embed"
	);
	assert_eq!(embed_pass(&vectors, &whole()).await, 2);
	assert_eq!(vectors.pending_count(&whole()).await.unwrap(), 0);
	assert_eq!(
		embed_pass(&vectors, &whole()).await,
		0,
		"a second pass does nothing"
	);

	let found = semantic_paths(&vectors, "vehicle", &whole()).await;
	assert_eq!(found.first().map(String::as_str), Some("notes/cars.md"));
	let found = semantic_paths(&vectors, "harbour bay", &whole()).await;
	assert_eq!(found.first().map(String::as_str), Some("notes/sea.md"));

	let hit = &vectors
		.search(&VectorSearch {
			vector: embed("sedan"),
			scope: whole(),
			limit: 1,
			..Default::default()
		})
		.await
		.unwrap()[0];
	assert_eq!(hit.text, "My automobile needs new tyres before winter.");
	assert_eq!(hit.kind, "markdown");

	let counts = vectors.counts().await.unwrap();
	assert_eq!((counts.documents, counts.chunks), (2, 2));
	assert_eq!(counts.model.as_deref(), Some("stub"));
}

#[tokio::test]
async fn a_changed_file_drops_out_until_embedded_again() {
	let mut f = Fixture::new().await;
	f.observe(&[("cars.md", 10)]).await;
	let (text, vectors) = f.indexes().await;
	read_texts(&text, &[("cars.md", "the automobile")]).await;
	embed_pass(&vectors, &whole()).await;
	assert_eq!(semantic_paths(&vectors, "car", &whole()).await, ["cars.md"]);

	// New bytes: the text row is stale until read, and so are its vectors.
	f.observe(&[("cars.md", 20)]).await;
	assert!(semantic_paths(&vectors, "car", &whole()).await.is_empty());
	read_texts(&text, &[("cars.md", "the puppy")]).await;
	assert_eq!(vectors.pending_count(&whole()).await.unwrap(), 1);
	embed_pass(&vectors, &whole()).await;
	assert_eq!(semantic_paths(&vectors, "dog", &whole()).await, ["cars.md"]);
	assert_eq!(
		vectors.counts().await.unwrap().chunks,
		1,
		"old chunks replaced"
	);
}

#[tokio::test]
async fn only_the_deep_scope_is_embedded_or_returned() {
	let mut f = Fixture::new().await;
	f.observe(&[("Documents/cars.md", 10), ("Library/cache/cars.md", 11)])
		.await;
	let (text, vectors) = f.indexes().await;
	read_texts(
		&text,
		&[
			("Documents/cars.md", "automobile"),
			("Library/cache/cars.md", "automobile"),
		],
	)
	.await;

	let documents = DeepScope::only(&["Documents"]);
	assert_eq!(vectors.pending_count(&documents).await.unwrap(), 1);
	assert_eq!(embed_pass(&vectors, &documents).await, 1);
	assert_eq!(
		semantic_paths(&vectors, "car", &whole()).await,
		["Documents/cars.md"]
	);

	// Widen, then release: vectors follow the scope.
	assert_eq!(embed_pass(&vectors, &whole()).await, 1);
	assert_eq!(semantic_paths(&vectors, "car", &whole()).await.len(), 2);
	let library = DeepScope::only(&["Library"]);
	assert_eq!(
		semantic_paths(&vectors, "car", &library).await,
		["Library/cache/cars.md"],
		"a search never returns what lies outside the scope, even before a prune"
	);
	assert_eq!(vectors.prune(&library).await.unwrap(), 1);
	assert_eq!(vectors.counts().await.unwrap().documents, 1);
	assert!(semantic_paths(&vectors, "car", &DeepScope::default())
		.await
		.is_empty());
}

#[tokio::test]
async fn transcripts_and_scenes_are_found_at_their_moment() {
	let mut f = Fixture::new().await;
	let uuids = f
		.observe(&[("clips/tour.mp4", 30), ("clips/drone.mp4", 31)])
		.await;
	f.db.set_content_identity(
		uuids[1],
		&sd_store::ContentIdentity {
			sampled_hash: Some("drone".into()),
			integrity_hash: None,
			size: Some(31),
			kind: None,
			kind_name: None,
		},
	)
	.await
	.unwrap();
	let (text, vectors) = f.indexes().await;
	let pending = text
		.pending_transcripts(&["mp4"], "clips/tour.mp4", 0, 10)
		.await
		.unwrap();
	let tour = &pending[0];
	text.write_transcript(&TranscriptDoc {
		record_uuid: tour.record_uuid,
		size: tour.size,
		mtime: tour.mtime,
		extension: tour.extension.clone(),
		language: Some("en".into()),
		model: "whisper-tiny".into(),
		duration_ms: Some(90_000),
		spans: vec![
			TranscriptSpan {
				start_ms: 0,
				end_ms: 4_000,
				text: "Welcome aboard.".into(),
			},
			TranscriptSpan {
				start_ms: 60_000,
				end_ms: 64_000,
				text: format!("{} the beacon was lit in 1902.", "and then ".repeat(120)),
			},
		],
		skipped: None,
	})
	.await
	.unwrap();
	text.write_descriptions(&DescriptionDoc {
		content_uuid: sd_store::uuid_for("drone"),
		kind: SCENE_KIND.into(),
		spans: vec![
			DescriptionSpan {
				start_ms: Some(0),
				end_ms: Some(5_000),
				text: "A dog runs on grass".into(),
			},
			DescriptionSpan {
				start_ms: Some(5_000),
				end_ms: Some(9_000),
				text: "Waves along the ocean coast".into(),
			},
		],
	})
	.await
	.unwrap();

	assert_eq!(embed_pass(&vectors, &whole()).await, 2);
	let hits = vectors
		.search(&VectorSearch {
			vector: embed("lighthouse"),
			scope: whole(),
			limit: 5,
			..Default::default()
		})
		.await
		.unwrap();
	assert_eq!(hits[0].external_id, "clips/tour.mp4");
	assert_eq!(hits[0].kind, "transcript");
	assert_eq!(
		hits[0].start_ms,
		Some(60_000),
		"the long span is its own chunk"
	);

	let hits = vectors
		.search(&VectorSearch {
			vector: embed("sea"),
			scope: whole(),
			kinds: vec![SCENE_KIND.into()],
			limit: 5,
			..Default::default()
		})
		.await
		.unwrap();
	assert_eq!(hits.len(), 1);
	assert_eq!(hits[0].external_id, "clips/drone.mp4");
	assert_eq!(
		(hits[0].start_ms, hits[0].end_ms),
		(Some(5_000), Some(9_000))
	);
	assert_eq!(hits[0].text, "Waves along the ocean coast");
}

#[tokio::test]
async fn hybrid_ranking_fuses_words_and_meaning() {
	let mut f = Fixture::new().await;
	f.observe(&[
		("both.md", 10),
		("words.md", 11),
		("meaning.md", 12),
		("unrelated.md", 13),
		("unembedded.md", 14),
	])
	.await;
	let (text, vectors) = f.indexes().await;
	read_texts(
		&text,
		&[
			("both.md", "The Q3 budget and spending review."),
			("words.md", "Q3 offsite agenda and venue."),
			("meaning.md", "Finance notes: costs went up."),
			("unrelated.md", "A puppy chased the ball."),
		],
	)
	.await;
	embed_pass(&vectors, &whole()).await;
	// Text that lands after the pass: found by its words alone.
	f.observe(&[("unembedded.md", 14)]).await;
	read_texts(&text, &[("unembedded.md", "Q3 budget draft")]).await;

	let hits = hybrid_search(
		&text,
		&vectors,
		&HybridSearch {
			query: "Q3 budget".into(),
			vector: embed("Q3 budget"),
			scope: whole(),
			limit: 10,
			snippet_tokens: 8,
			..Default::default()
		},
	)
	.await
	.unwrap();
	let order: Vec<&str> = hits.iter().map(|h| h.external_id.as_str()).collect();
	assert_eq!(order[0], "both.md", "{order:?}");
	assert!(order.contains(&"words.md"));
	assert!(order.contains(&"meaning.md"));
	let unembedded = hits
		.iter()
		.find(|h| h.external_id == "unembedded.md")
		.unwrap();
	assert!(unembedded.similarity.is_none() && unembedded.lexical_rank.is_some());
	let meaning = hits.iter().find(|h| h.external_id == "meaning.md").unwrap();
	assert!(meaning.lexical_rank.is_none() && meaning.similarity.is_some());
	assert_eq!(meaning.snippet[0].text, "Finance notes: costs went up.");
	let both = &hits[0];
	assert!(
		both.snippet.iter().any(|part| part.matched),
		"word hits keep their highlights"
	);

	let page = hybrid_search(
		&text,
		&vectors,
		&HybridSearch {
			query: "Q3 budget".into(),
			vector: embed("Q3 budget"),
			scope: whole(),
			limit: 2,
			offset: 1,
			snippet_tokens: 8,
			..Default::default()
		},
	)
	.await
	.unwrap();
	assert_eq!(page[0].external_id, order[1]);
}

#[tokio::test]
async fn another_model_drops_every_vector() {
	let mut f = Fixture::new().await;
	f.observe(&[("a.md", 10)]).await;
	let (text, vectors) = f.indexes().await;
	read_texts(&text, &[("a.md", "automobile")]).await;
	embed_pass(&vectors, &whole()).await;
	assert!(!vectors.use_model("stub", DIM).await.unwrap());
	assert!(vectors.use_model("other", 384).await.unwrap());
	assert_eq!(vectors.counts().await.unwrap().documents, 0);
	assert_eq!(vectors.pending_count(&whole()).await.unwrap(), 1);
}

#[tokio::test]
async fn files_outside_the_folder_never_crowd_out_the_ones_inside() {
	let mut f = Fixture::new().await;
	let mut files: Vec<(String, i64)> = (0..80)
		.map(|i| (format!("Other/car{i}.md"), 100 + i))
		.collect();
	files.push(("Notes/ride.md".to_string(), 10));
	let refs: Vec<(&str, i64)> = files.iter().map(|(p, s)| (p.as_str(), *s)).collect();
	f.observe(&refs).await;
	let (text, vectors) = f.indexes().await;
	let mut texts: Vec<(String, String)> = (0..80)
		.map(|i| (format!("Other/car{i}.md"), "car car automobile".to_string()))
		.collect();
	texts.push((
		"Notes/ride.md".into(),
		"a sedan ride to the coast by the sea".into(),
	));
	let refs: Vec<(&str, &str)> = texts
		.iter()
		.map(|(p, b)| (p.as_str(), b.as_str()))
		.collect();
	read_texts(&text, &refs).await;
	embed_pass(&vectors, &whole()).await;

	let hits = vectors
		.search(&VectorSearch {
			vector: embed("car"),
			under: "Notes".into(),
			scope: whole(),
			limit: 1,
			..Default::default()
		})
		.await
		.unwrap();
	assert_eq!(hits.len(), 1);
	assert_eq!(hits[0].external_id, "Notes/ride.md");
	let kinds = vectors
		.search(&VectorSearch {
			vector: embed("car"),
			scope: whole(),
			kinds: vec!["transcript".into()],
			limit: 5,
			..Default::default()
		})
		.await
		.unwrap();
	assert!(kinds.is_empty(), "a kind filter applies before ranking");
}

#[tokio::test]
async fn hybrid_search_keeps_to_the_scope_for_words_too() {
	let mut f = Fixture::new().await;
	f.observe(&[("Documents/a.md", 10), ("Library/b.md", 11)])
		.await;
	let (text, vectors) = f.indexes().await;
	read_texts(
		&text,
		&[
			("Documents/a.md", "budget notes"),
			("Library/b.md", "budget cache"),
		],
	)
	.await;
	let search = |scope: DeepScope| HybridSearch {
		query: "budget".into(),
		vector: embed("budget"),
		scope,
		limit: 10,
		snippet_tokens: 8,
		..Default::default()
	};
	let hits = hybrid_search(&text, &vectors, &search(DeepScope::only(&["Documents"])))
		.await
		.unwrap();
	let paths: Vec<&str> = hits.iter().map(|h| h.external_id.as_str()).collect();
	assert_eq!(paths, ["Documents/a.md"]);
	assert!(
		hybrid_search(&text, &vectors, &search(DeepScope::default()))
			.await
			.unwrap()
			.is_empty()
	);
}

#[tokio::test]
async fn a_rewritten_description_is_embedded_again_though_its_row_ids_are_reused() {
	let mut f = Fixture::new().await;
	let uuids = f.observe(&[("photos/pier.jpg", 30)]).await;
	f.db.set_content_identity(
		uuids[0],
		&sd_store::ContentIdentity {
			sampled_hash: Some("pier".into()),
			integrity_hash: None,
			size: Some(30),
			kind: None,
			kind_name: None,
		},
	)
	.await
	.unwrap();
	let (text, vectors) = f.indexes().await;
	let caption = |body: &str| DescriptionDoc {
		content_uuid: sd_store::uuid_for("pier"),
		kind: sd_store::CAPTION_KIND.into(),
		spans: vec![DescriptionSpan {
			start_ms: None,
			end_ms: None,
			text: body.into(),
		}],
	};
	text.write_descriptions(&caption("a dog on the pier"))
		.await
		.unwrap();
	assert_eq!(embed_pass(&vectors, &whole()).await, 1);
	text.write_descriptions(&caption("a car on the pier"))
		.await
		.unwrap();
	assert_eq!(vectors.pending_count(&whole()).await.unwrap(), 1);
	embed_pass(&vectors, &whole()).await;
	assert_eq!(
		semantic_paths(&vectors, "vehicle", &whole()).await,
		["photos/pier.jpg"]
	);
}

#[tokio::test]
async fn tied_scores_page_in_one_order() {
	let mut f = Fixture::new().await;
	let files: Vec<(String, i64)> = (0..40).map(|i| (format!("car{i}.md"), 100 + i)).collect();
	let refs: Vec<(&str, i64)> = files.iter().map(|(p, s)| (p.as_str(), *s)).collect();
	f.observe(&refs).await;
	let (text, vectors) = f.indexes().await;
	let texts: Vec<(&str, &str)> = files
		.iter()
		.map(|(p, _)| (p.as_str(), "automobile"))
		.collect();
	// Later files are read first, so chunk order runs against record order.
	read_texts(&text, &texts[20..]).await;
	read_texts(&text, &texts[..20]).await;
	embed_pass(&vectors, &whole()).await;

	let mut paged = Vec::new();
	for page in 0..4 {
		let hits = hybrid_search(
			&text,
			&vectors,
			&HybridSearch {
				query: "vehicle".into(),
				vector: embed("vehicle"),
				scope: whole(),
				limit: 10,
				offset: page * 10,
				..Default::default()
			},
		)
		.await
		.unwrap();
		paged.extend(hits.into_iter().map(|hit| hit.external_id));
	}
	let distinct: std::collections::HashSet<&String> = paged.iter().collect();
	assert_eq!(paged.len(), 40);
	assert_eq!(distinct.len(), 40, "no file repeats across pages");
}
