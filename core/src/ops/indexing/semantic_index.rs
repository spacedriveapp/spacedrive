//! Filling the semantic vector index.
//!
//! This pass runs behind the text pass, the transcription job and sidecar
//! descriptions. It claims the documents in `text.db` whose vectors are
//! missing or were made from other text, cuts each into chunks, embeds them
//! and writes the vectors to `vectors.db` beside the source store.
//!
//! It follows the rules every background pass keeps. One pass per source at a
//! time, deduplicated on the root. No job is started when nothing is pending,
//! and a background pass that embedded nothing leaves no row. Sidecar writes
//! arrive one file at a time, so they mark the source and one pass follows
//! after a quiet window rather than one per sidecar. Only the deep scope is
//! embedded, and the job runs at low priority with the model on the
//! background runtime the hashing reads use.
//!
//! A source with no embedder (no model downloaded, or a build without local
//! inference) gets no pass at all.

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::{Arc, Mutex, OnceLock},
	time::Duration,
};

use sd_store::{Origin, PendingVectors, VectorIndex};
use serde::{Deserialize, Serialize};

use crate::{
	infra::job::{generic_progress::GenericProgress, prelude::*, types::JobPriority},
	ops::search::semantic::embedder::{self, Embedder},
};

/// Documents claimed per round.
const BATCH_DOCS: usize = 16;
/// Chunks embedded per model call, so a pause is seen between calls.
const BATCH_CHUNKS: usize = 16;
/// How long sidecar writes to one source are gathered into one pass.
const SIDECAR_WINDOW: Duration = Duration::from_secs(300);

/// Embeds the documents of a source whose vectors are missing or stale.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct SemanticIndexJob {
	/// The source's root, which is how its store is found.
	root: PathBuf,
	/// Dispatched behind a background pass rather than a person.
	#[serde(default)]
	background: bool,
}

impl SemanticIndexJob {
	pub fn new(root: PathBuf, background: bool) -> Self {
		Self { root, background }
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticIndexOutput {
	/// Documents whose vectors were written.
	pub embedded: u64,
	/// Chunks embedded across them.
	pub chunks: u64,
	/// Documents the model failed on, left pending for the next pass.
	pub failed: u64,
	/// Documents whose vectors were removed: gone, rewritten, or outside
	/// the scope.
	pub pruned: u64,
}

impl From<SemanticIndexOutput> for JobOutput {
	fn from(output: SemanticIndexOutput) -> Self {
		JobOutput::custom(output)
	}
}

impl Job for SemanticIndexJob {
	const NAME: &'static str = "semantic_index";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Index file meaning");
}

impl crate::infra::job::traits::DynJob for SemanticIndexJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn dedup_key(&self) -> Option<String> {
		Some(format!("pass:{}", self.root.display()))
	}

	fn should_emit_events(&self) -> bool {
		!self.background
	}
}

#[async_trait::async_trait]
impl JobHandler for SemanticIndexJob {
	type Output = SemanticIndexOutput;

	fn keep_record(&self, output: &Self::Output) -> bool {
		!self.background || output.embedded + output.failed + output.pruned > 0
	}

	fn finished_rows_kept(&self) -> Option<u64> {
		self.background
			.then_some(super::content_identity::BACKGROUND_HISTORY)
	}

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let volume_index = ctx.library().core_context().volume_index();
		if let Some(reason) = volume_index.dispatch_refusal(&self.root) {
			return Err(JobError::execution(reason));
		}
		let Some(store) = volume_index.store_for(&self.root).await else {
			return Ok(SemanticIndexOutput::default());
		};
		let embedder = match embedder::resolve(ctx.library().core_context()).await {
			Ok(embedder) => embedder,
			Err(reason) => {
				ctx.log(format!("Semantic search is unavailable: {reason}"));
				return Ok(SemanticIndexOutput::default());
			}
		};
		let vectors = store.vector_index().await.map_err(|e| e.to_string())?;
		let subtrees = store.deep_subtrees().await.map_err(|e| e.to_string())?;
		let label = super::content_identity::source_label(&self.root);

		let (output, stopped) = embed_pending(
			&vectors,
			&subtrees,
			embedder,
			|| async { ctx.check_interrupt().await.is_err() },
			|done, outstanding| {
				ctx.progress(Progress::generic(GenericProgress::new(
					if outstanding > 0 {
						(done as f32 / outstanding as f32).min(1.0)
					} else {
						1.0
					},
					"Indexing file meaning",
					format!("{label} — {done} of {outstanding} documents"),
				)))
			},
		)
		.await
		.map_err(|e| e.to_string())?;
		if stopped {
			return Err(JobError::Interrupted);
		}
		ctx.log(format!(
			"Embedded {} documents in {} chunks, {} failed, {} pruned",
			output.embedded, output.chunks, output.failed, output.pruned
		));
		Ok(output)
	}
}

/// Embed every pending document in `subtrees`, pruning stale vectors first.
///
/// Stops early, with what it wrote kept, once `interrupted` says so, and
/// then returns `true` beside the counts. A
/// document the model fails on is counted and left pending. The claim walks
/// forward by `(origin, doc_id)` and sweeps once more from the start, so a
/// document written behind the cursor during the pass is still embedded.
pub async fn embed_pending<F: std::future::Future<Output = bool>>(
	vectors: &VectorIndex,
	subtrees: &[String],
	embedder: Arc<dyn Embedder>,
	interrupted: impl Fn() -> F,
	progress: impl Fn(u64, u64),
) -> anyhow::Result<(SemanticIndexOutput, bool)> {
	let probe = {
		let embedder = embedder.clone();
		background(move || embedder.embed_query("dimension")).await?
	};
	vectors.use_model(embedder.model(), probe.len()).await?;
	let mut output = SemanticIndexOutput {
		pruned: vectors.prune(subtrees).await?,
		..Default::default()
	};
	let outstanding = vectors.pending_count(subtrees).await?;
	let mut attempted: HashSet<(Origin, i64, String)> = HashSet::new();
	let mut cursor = (Origin::Text, 0);
	let mut swept_again = false;
	let mut done = 0u64;
	loop {
		let claimed = vectors.pending(subtrees, cursor, BATCH_DOCS).await?;
		let Some(last) = claimed.last() else {
			if swept_again || cursor == (Origin::Text, 0) {
				break;
			}
			swept_again = true;
			cursor = (Origin::Text, 0);
			continue;
		};
		cursor = (last.origin, last.doc_id);
		for doc in claimed {
			if !attempted.insert((doc.origin, doc.doc_id, doc.stamp.clone())) {
				continue;
			}
			if interrupted().await {
				return Ok((output, true));
			}
			match embed_document(vectors, &doc, &embedder).await {
				Ok(chunks) => {
					output.embedded += 1;
					output.chunks += chunks;
				}
				Err(error) => {
					tracing::debug!(origin = ?doc.origin, doc = doc.doc_id, %error, "could not embed a document");
					output.failed += 1;
				}
			}
			done += 1;
			progress(done, outstanding);
		}
	}
	Ok((output, false))
}

async fn embed_document(
	vectors: &VectorIndex,
	doc: &PendingVectors,
	embedder: &Arc<dyn Embedder>,
) -> anyhow::Result<u64> {
	let chunks = vectors.chunks(doc).await?;
	let mut embedded = Vec::with_capacity(chunks.len());
	for batch in chunks.chunks(BATCH_CHUNKS) {
		let texts: Vec<String> = batch.iter().map(|chunk| chunk.text.clone()).collect();
		let embedder = embedder.clone();
		let out = background(move || {
			let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
			embedder.embed_passages(&texts)
		})
		.await?;
		if out.len() != batch.len() {
			anyhow::bail!("{} chunks but {} vectors", batch.len(), out.len());
		}
		embedded.extend(out);
	}
	vectors.write(doc, &chunks, &embedded).await?;
	Ok(chunks.len() as u64)
}

/// Run model work on the background-priority runtime the hashing reads use.
async fn background<T: Send + 'static>(
	work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> anyhow::Result<T> {
	let task = super::content_identity::hashing_runtime().spawn_blocking(work);
	match task.await {
		Ok(result) => result.map_err(anyhow::Error::msg),
		Err(error) => std::panic::resume_unwind(error.into_panic()),
	}
}

/// Queue a semantic pass for a source when it has documents to embed.
///
/// Called when a text pass or a transcription job finishes. Nothing is
/// queued without an installed embedder or pending documents, so a device
/// that never turned semantic search on never sees the job.
pub async fn index_vectors_soon(
	library: &Arc<crate::library::Library>,
	root: &Path,
	background: bool,
) {
	let context = library.core_context();
	if !embedder::installed(context).await {
		return;
	}
	let volume_index = context.volume_index();
	if !volume_index
		.source_config_at(root)
		.is_some_and(|config| config.identify_content)
	{
		return;
	}
	let Some(store) = volume_index.store_for(root).await else {
		return;
	};
	let pending = async {
		let subtrees = store.deep_subtrees().await?;
		anyhow::Ok(store.vector_index().await?.pending_count(&subtrees).await?)
	};
	match pending.await {
		Ok(0) => return,
		Ok(_) => {}
		Err(error) => {
			tracing::warn!(root = %root.display(), %error, "could not count documents waiting for the vector index");
			return;
		}
	}
	if let Err(error) = library
		.jobs()
		.dispatch_with_priority(
			SemanticIndexJob::new(root.to_path_buf(), background),
			JobPriority::LOW,
			None,
		)
		.await
	{
		tracing::warn!(root = %root.display(), %error, "could not start the semantic index pass");
	}
}

/// Gather sidecar description writes to a source into one pass after a
/// quiet window. Extensions write sidecars one file at a time; a pass per
/// sidecar would leave a job row per photo.
pub fn index_vectors_after_sidecars(library: &crate::library::Library, root: &Path) {
	static MARKED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
	let marked = MARKED.get_or_init(Default::default);
	if !marked.lock().unwrap().insert(root.to_path_buf()) {
		return;
	}
	let (context, library_id, root) = (
		library.core_context().clone(),
		library.id(),
		root.to_path_buf(),
	);
	tokio::spawn(async move {
		tokio::time::sleep(SIDECAR_WINDOW).await;
		marked.lock().unwrap().remove(&root);
		if let Some(library) = context.libraries().await.get_library(library_id).await {
			index_vectors_soon(&library, &root, true).await;
		}
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	use sd_store::file::{FileKind, FileWrite, Ledger, Observation};
	use sd_store::{ContentTextIndex, SourceManager, TextDoc, TextFilter};

	/// Words count on hashed axes; enough for a pass to have vectors.
	struct Stub;

	impl Embedder for Stub {
		fn model(&self) -> &str {
			"stub"
		}
		fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
			if texts.iter().any(|t| t.contains("poison")) {
				return Err("the model choked".into());
			}
			Ok(texts.iter().map(|t| self.embed_query(t).unwrap()).collect())
		}
		fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
			let mut v = vec![0.0f32; 8];
			for word in text.split_whitespace() {
				v[word.len() % 8] += 1.0;
			}
			let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-6);
			Ok(v.iter().map(|x| x / norm).collect())
		}
	}

	/// A source store with one text row per file, written the way a text
	/// pass writes them. Parents are created from the paths.
	async fn source_with(files: &[(String, String)]) -> (tempfile::TempDir, std::path::PathBuf) {
		let dir = tempfile::tempdir().unwrap();
		let db = SourceManager::new(dir.path().to_path_buf())
			.ensure("s", &sd_store::filesystem_schema())
			.await
			.unwrap();
		db.begin_sync().await.unwrap();
		let mut ledger = Ledger::load(db.pool()).await.unwrap();
		let mut writes = Vec::new();
		let mut seen = HashSet::new();
		for (path, _) in files {
			let parts: Vec<&str> = path.split('/').collect();
			for depth in 1..=parts.len() {
				let at = parts[..depth].join("/");
				if !seen.insert(at.clone()) {
					continue;
				}
				let file = depth == parts.len();
				let observation = Observation {
					external_id: at.clone(),
					kind: if file {
						FileKind::File
					} else {
						FileKind::Directory
					},
					name: parts[depth - 1].into(),
					size: 5,
					mtime: 1,
					created: None,
					accessed: None,
					inode: None,
					mode: None,
					uid: None,
					gid: None,
					link_target: None,
					extension: file
						.then(|| at.rsplit_once('.').map(|(_, e)| e.into()))
						.flatten(),
					is_hidden: false,
					identity: None,
				};
				let resolution = ledger.resolve(&observation);
				let parent_uuid = at
					.rsplit_once('/')
					.and_then(|(parent, _)| ledger.uuid_of(parent));
				writes.push(FileWrite {
					resolution,
					parent_uuid,
					observation,
				});
			}
		}
		db.apply_files(&writes, &[], &[], None).await.unwrap();
		let source = dir.path().join("s");
		let text = ContentTextIndex::open(&source).await.unwrap();
		let pending = text
			.pending(&TextFilter::everywhere(&["md"]), 0, 1_000_000)
			.await
			.unwrap();
		let docs: Vec<TextDoc> = pending
			.iter()
			.map(|p| TextDoc {
				record_uuid: p.record_uuid,
				size: p.size,
				mtime: p.mtime,
				extension: p.extension.clone(),
				kind: "markdown".into(),
				text: files
					.iter()
					.find(|(path, _)| *path == p.external_id)
					.map(|(_, body)| body.clone())
					.unwrap_or_default(),
				truncated: false,
				skipped: None,
			})
			.collect();
		for batch in docs.chunks(256) {
			text.write(batch).await.unwrap();
		}
		(dir, source)
	}

	#[tokio::test]
	async fn a_pass_embeds_the_scope_once_and_leaves_failures_pending() {
		let files: Vec<(String, String)> = [
			("Documents/a.md", "plain words here"),
			("Documents/bad.md", "poison words"),
			("Library/b.md", "plain words here"),
		]
		.into_iter()
		.map(|(p, b)| (p.to_string(), b.to_string()))
		.collect();
		let (_dir, source) = source_with(&files).await;
		let vectors = VectorIndex::open(&source).await.unwrap();
		let scope = vec!["Documents".to_string()];

		let seen = std::sync::Mutex::new(Vec::new());
		let (output, _) = embed_pending(
			&vectors,
			&scope,
			Arc::new(Stub),
			|| async { false },
			|done, of| seen.lock().unwrap().push((done, of)),
		)
		.await
		.unwrap();
		assert_eq!((output.embedded, output.failed, output.chunks), (1, 1, 1));
		assert_eq!(seen.into_inner().unwrap().last(), Some(&(2, 2)));
		assert_eq!(
			vectors.pending_count(&scope).await.unwrap(),
			1,
			"the failure waits"
		);
		assert_eq!(
			vectors.counts().await.unwrap().model.as_deref(),
			Some("stub")
		);

		let (again, _) = embed_pending(
			&vectors,
			&scope,
			Arc::new(Stub),
			|| async { false },
			|_, _| {},
		)
		.await
		.unwrap();
		assert_eq!((again.embedded, again.failed), (0, 1));

		let (stopped, interrupted) = embed_pending(
			&vectors,
			&[String::new()],
			Arc::new(Stub),
			|| async { true },
			|_, _| {},
		)
		.await
		.unwrap();
		assert!(interrupted);
		assert_eq!(
			stopped.embedded, 0,
			"an interrupted pass writes nothing more"
		);
	}

	/// Embedding cost and index size for 1,000 documents with the shipped
	/// model. Needs the model files under SD_AI_MODELS_DIR and a folder of
	/// text files in SD_BENCH_CORPUS. Run with
	/// cargo test -p sd-core --features ai-local --release --lib semantic_benchmark -- --ignored --nocapture
	#[cfg(feature = "ai-local")]
	#[tokio::test(flavor = "multi_thread")]
	#[ignore]
	async fn semantic_benchmark() {
		use crate::ops::ai::{catalog, local, registry::TextEmbedder, Provider};

		struct Real(Arc<dyn TextEmbedder>);
		impl Embedder for Real {
			fn model(&self) -> &str {
				"multilingual-e5-small"
			}
			fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
				self.0.embed_passages(texts).map_err(|e| e.to_string())
			}
			fn embed_query(&self, text: &str) -> Result<Vec<f32>, String> {
				Ok(self.0.embed(&[text]).map_err(|e| e.to_string())?.remove(0))
			}
		}

		let (Some(models), Some(corpus)) = (
			std::env::var_os("SD_AI_MODELS_DIR"),
			std::env::var_os("SD_BENCH_CORPUS"),
		) else {
			return;
		};
		let spec = catalog::find("multilingual-e5-small").unwrap();
		let Provider::TextEmbedding(inner) =
			local::load(spec, &Path::new(&models).join(spec.id)).unwrap()
		else {
			panic!("not a text embedder");
		};
		let mut paths: Vec<PathBuf> = std::fs::read_dir(&corpus)
			.unwrap()
			.filter_map(|e| e.ok().map(|e| e.path()))
			.collect();
		paths.sort();
		let files: Vec<(String, String)> = paths
			.iter()
			.take(1000)
			.enumerate()
			.map(|(i, p)| {
				let text = std::fs::read_to_string(p).unwrap_or_default();
				(format!("Documents/doc{i}.md"), text)
			})
			.collect();
		let bytes: usize = files.iter().map(|(_, b)| b.len()).sum();
		let (_dir, source) = source_with(&files).await;
		let vectors = VectorIndex::open(&source).await.unwrap();
		let everything = [String::new()];

		let started = std::time::Instant::now();
		let (output, _) = embed_pending(
			&vectors,
			&everything,
			Arc::new(Real(inner.clone())),
			|| async { false },
			|_, _| {},
		)
		.await
		.unwrap();
		let embed_time = started.elapsed();
		let again_started = std::time::Instant::now();
		let (again, _) = embed_pending(
			&vectors,
			&everything,
			Arc::new(Real(inner.clone())),
			|| async { false },
			|_, _| {},
		)
		.await
		.unwrap();
		let again_time = again_started.elapsed();
		vectors.close().await;
		let vectors = VectorIndex::open(&source).await.unwrap();
		let size = std::fs::metadata(source.join(sd_store::VECTOR_INDEX_FILE))
			.unwrap()
			.len();
		let search = sd_store::VectorSearch {
			vector: inner
				.embed(&["how are files kept in sync between devices"])
				.unwrap()
				.remove(0),
			subtrees: everything.to_vec(),
			limit: 20,
			..Default::default()
		};
		let cold_started = std::time::Instant::now();
		vectors.search(&search).await.unwrap();
		let cold = cold_started.elapsed();
		let warm_started = std::time::Instant::now();
		vectors.search(&search).await.unwrap();
		let warm = warm_started.elapsed();

		eprintln!(
			"documents {} text {:.1} MB chunks {} embedded in {:.1} s ({:.1} ms a chunk) \
			 vectors.db {:.2} MB ({} B a chunk) no-op pass {} ms search cold {} ms warm {} ms cores {}",
			output.embedded,
			bytes as f64 / 1e6,
			output.chunks,
			embed_time.as_secs_f64(),
			embed_time.as_secs_f64() * 1000.0 / output.chunks.max(1) as f64,
			size as f64 / 1e6,
			size / output.chunks.max(1),
			again_time.as_millis(),
			cold.as_millis(),
			warm.as_millis(),
			std::thread::available_parallelism().map_or(0, |n| n.get()),
		);
		assert_eq!(again.embedded, 0);
		assert!(output.embedded > 900);
	}
}
