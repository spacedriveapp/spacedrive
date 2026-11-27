//! Filling the content text index.
//!
//! Identification runs after every walk, watcher nudge and returned volume,
//! because each of those leaves records whose bytes changed. This pass runs
//! behind it, claims the text files whose row in the content text index is
//! missing or stamped with other bytes, extracts their text on the
//! background-priority runtime the hashing reads use, and writes it to the
//! sidecar beside the source store.
//!
//! The work queue is the index itself (`sd_store::content_text`), so the
//! pass resumes for free and reads a file again only when its size or mtime
//! changed. A rename changes neither and costs nothing. Rows for deleted
//! files are pruned at the start of every pass; until then no search can
//! reach them, because a search joins through the record. A source added
//! without content identification gets no pass, since content reads are
//! what that setting turns off.
//!
//! A pass reads only the source's deep scope ([`super::deep_scope`]), by
//! default the home folder's standard user folders, and never a hidden
//! folder. Metadata capture covers the whole source; the text of a
//! home folder's caches, toolchains, logs and agent transcripts once grew
//! the index past the size of everything a person would search.

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_store::{PendingText, TextDoc, TextFilter};
use serde::{Deserialize, Serialize};

use super::text_extract::{self, Extraction, TextKind};
use crate::infra::job::{generic_progress::GenericProgress, prelude::*, types::JobPriority};

/// Files claimed per round. Each is up to a megabyte of text, so the claim
/// bounds what one transaction writes.
const BATCH_SIZE: usize = 64;

/// Extracts text into the content text index for the files of a source
/// whose row is missing or out of date.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ContentTextJob {
	/// The source's root, which is how its store is found.
	root: PathBuf,
	/// Dispatched behind a background pass rather than a person, and silent
	/// on the event bus like it.
	#[serde(default)]
	background: bool,
	/// Drop the index before reading, which `search.content.rebuild` asks
	/// for. Cleared once done, so a resumed pass does not drop it again.
	#[serde(default)]
	rebuild: bool,
}

impl ContentTextJob {
	pub fn new(root: PathBuf, background: bool) -> Self {
		Self {
			root,
			background,
			rebuild: false,
		}
	}

	/// A pass that drops the source's index first and reads every file.
	///
	/// It deduplicates only onto another rebuild, never onto an ordinary
	/// pass, so the clear and the pass that refills it are always one job.
	/// An ordinary pass running beside it sees the epoch change and starts
	/// over; both write the same rows.
	pub fn rebuild(root: PathBuf) -> Self {
		Self {
			root,
			background: false,
			rebuild: true,
		}
	}
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentTextOutput {
	/// Files whose text was written.
	pub indexed: u64,
	/// Files read with nothing to index: binary, empty, or unparseable.
	pub skipped: u64,
	/// Files that could not be opened, or cloud placeholders left unread.
	/// Left pending for the next pass.
	pub unreadable: u64,
	/// Rows removed because their file is gone.
	pub pruned: u64,
}

impl From<ContentTextOutput> for JobOutput {
	fn from(output: ContentTextOutput) -> Self {
		JobOutput::custom(output)
	}
}

impl Job for ContentTextJob {
	const NAME: &'static str = "content_text";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Index file text");
}

impl crate::infra::job::traits::DynJob for ContentTextJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	/// One pass per source at a time; the work queue is shared.
	fn dedup_key(&self) -> Option<String> {
		let kind = if self.rebuild { "rebuild" } else { "pass" };
		Some(format!("{kind}:{}", self.root.display()))
	}

	fn should_emit_events(&self) -> bool {
		!self.background
	}
}

#[async_trait::async_trait]
impl JobHandler for ContentTextJob {
	type Output = ContentTextOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let volume_index = ctx.library().core_context().volume_index();
		if let Some(reason) = volume_index.dispatch_refusal(&self.root) {
			return Err(JobError::execution(reason));
		}
		let Some(store) = volume_index.store_for(&self.root).await else {
			ctx.log(format!(
				"No source store at {}; nothing to index",
				self.root.display()
			));
			return Ok(ContentTextOutput::default());
		};
		// The pass reads the record store; claim what has landed.
		store.flush().await.map_err(|e| e.to_string())?;
		let index = store.text_index().await.map_err(|e| e.to_string())?;
		let mut filter = text_filter(&store).await.map_err(|e| e.to_string())?;
		if self.rebuild {
			index.clear().await.map_err(|e| e.to_string())?;
			self.rebuild = false;
			ctx.log("Dropped the text index; reading every file again");
		}

		let mut output = ContentTextOutput {
			pruned: index.prune(&filter).await.map_err(|e| e.to_string())?,
			..Default::default()
		};
		let outstanding = index
			.pending_count(&filter)
			.await
			.map_err(|e| e.to_string())?;
		ctx.log(format!(
			"{outstanding} files to index, {} rows pruned",
			output.pruned
		));

		let label = super::content_identity::source_label(&self.root);
		let mut done = 0u64;
		let mut cursor = 0i64;
		// The same second sweep the metadata pass makes: a file changed
		// behind the cursor is picked up, and one that could not be read is
		// not tried twice under the same stamp.
		let mut swept_again = false;
		let mut attempted: HashSet<(i64, Option<i64>, Option<i64>, String)> = HashSet::new();
		// A rebuild that lands while this pass runs clears the rows it wrote
		// and deduplicates onto it, so a changed epoch starts the pass over.
		let mut epoch = index.epoch().await.map_err(|e| e.to_string())?;

		loop {
			ctx.check_interrupt().await?;
			let now = index.epoch().await.map_err(|e| e.to_string())?;
			if now != epoch {
				epoch = now;
				cursor = 0;
				swept_again = false;
				attempted.clear();
				ctx.log("The index was cleared; reading every file again");
			}
			// A scope changed while this pass runs deduplicates onto it, so
			// the pass takes the new scope rather than finishing on the old.
			let current = text_filter(&store).await.map_err(|e| e.to_string())?;
			if current != filter {
				filter = current;
				output.pruned += index.prune(&filter).await.map_err(|e| e.to_string())?;
				cursor = 0;
				swept_again = false;
				// A folder released and kept again lost its rows to the prune.
				attempted.clear();
				ctx.log("The deep scope changed; reading under the new one");
			}
			let claimed = index
				.pending(&filter, cursor, BATCH_SIZE)
				.await
				.map_err(|e| e.to_string())?;
			let Some(last) = claimed.last() else {
				let cleared = index.epoch().await.map_err(|e| e.to_string())? != epoch;
				let rescoped = text_filter(&store).await.map_err(|e| e.to_string())? != filter;
				if !cleared && !rescoped && (swept_again || cursor == 0) {
					break;
				}
				swept_again = true;
				cursor = 0;
				continue;
			};
			cursor = last.rowid;
			let batch: Vec<PendingText> = claimed
				.into_iter()
				.filter(|file| {
					attempted.insert((file.rowid, file.size, file.mtime, file.extension.clone()))
				})
				.collect();
			if batch.is_empty() {
				continue;
			}
			done += batch.len() as u64;

			let (docs, unreadable) = extract_batch(self.root.clone(), batch).await;
			output.unreadable += unreadable;
			output.skipped += docs.iter().filter(|doc| doc.skipped.is_some()).count() as u64;
			output.indexed += docs.iter().filter(|doc| doc.skipped.is_none()).count() as u64;
			index.write(&docs).await.map_err(|e| e.to_string())?;

			ctx.progress(Progress::generic(GenericProgress::new(
				if outstanding > 0 {
					(done as f32 / outstanding as f32).min(1.0)
				} else {
					1.0
				},
				"Indexing file text",
				format!("{label} — {done} of {outstanding} files"),
			)));
		}

		let reclaimed = index.reclaim().await.map_err(|e| e.to_string())?;
		ctx.log(format!(
			"Indexed {} files, {} with nothing to index, {} unreadable, {reclaimed} bytes given back",
			output.indexed, output.skipped, output.unreadable
		));
		Ok(output)
	}
}

/// Extract a batch on the background-priority runtime.
///
/// A cloud placeholder is never opened, because opening it downloads it. It
/// gets no row and is counted unreadable, so it is claimed again once the
/// file is on this device. A file that cannot be opened is treated the same.
async fn extract_batch(root: PathBuf, batch: Vec<PendingText>) -> (Vec<TextDoc>, u64) {
	let task = super::content_identity::hashing_runtime().spawn_blocking(move || {
		let mut docs = Vec::with_capacity(batch.len());
		let mut unreadable = 0;
		for file in batch {
			let Some(kind) = TextKind::for_extension(&file.extension) else {
				continue;
			};
			let path = root.join(&file.external_id);
			let name = file.external_id.rsplit('/').next().unwrap_or_default();
			if let Some(reason) = text_extract::skip_by_name(name) {
				docs.push(TextDoc {
					record_uuid: file.record_uuid,
					size: file.size,
					mtime: file.mtime,
					extension: file.extension.clone(),
					kind: kind.as_str().to_string(),
					text: String::new(),
					truncated: false,
					skipped: Some(reason.to_string()),
				});
				continue;
			}
			if super::metadata::refuse_dataless_blocking(&path).is_err() {
				unreadable += 1;
				continue;
			}
			let cap = text_extract::text_cap(kind, &file.extension);
			let (text, truncated, skipped) = match text_extract::extract(&path, kind, cap) {
				Ok(Extraction::Text { text, truncated }) => (text, truncated, None),
				Ok(Extraction::Skipped(reason)) => (String::new(), false, Some(reason)),
				Err(error) => {
					tracing::debug!(path = %path.display(), %error, "could not read for the text index");
					unreadable += 1;
					continue;
				}
			};
			docs.push(TextDoc {
				record_uuid: file.record_uuid,
				size: file.size,
				mtime: file.mtime,
				extension: file.extension.clone(),
				kind: kind.as_str().to_string(),
				text,
				truncated,
				skipped,
			});
		}
		(docs, unreadable)
	});
	match task.await {
		Ok(result) => result,
		Err(error) => std::panic::resume_unwind(error.into_panic()),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn pending(name: &str) -> PendingText {
		PendingText {
			record_uuid: uuid::Uuid::now_v7(),
			rowid: 1,
			external_id: name.to_string(),
			size: Some(5),
			mtime: Some(1),
			extension: name.rsplit_once('.').unwrap().1.to_string(),
		}
	}

	#[tokio::test]
	async fn a_cloud_placeholder_is_never_opened() {
		let dir = tempfile::tempdir().unwrap();
		std::fs::write(dir.path().join("here.md"), "local words").unwrap();
		std::fs::write(dir.path().join("cloud.md"), "never read").unwrap();
		std::fs::write(dir.path().join("blob.txt"), b"a\0b").unwrap();
		super::super::metadata::dataless_for_tests::mark(&dir.path().join("cloud.md"));

		let (docs, unreadable) = extract_batch(
			dir.path().to_path_buf(),
			vec![
				pending("here.md"),
				pending("cloud.md"),
				pending("blob.txt"),
				pending("gone.md"),
			],
		)
		.await;

		assert_eq!(unreadable, 2, "the placeholder and the missing file");
		assert_eq!(docs.len(), 2);
		assert_eq!(docs[0].text, "local words");
		assert_eq!(docs[0].kind, "markdown");
		assert_eq!(docs[0].size, Some(5));
		assert_eq!(docs[1].skipped.as_deref(), Some("binary"));
		assert!(docs[1].text.is_empty());
	}
}

/// Queue a text pass for a source when it has files to read.
///
/// Called at the end of every identification pass and when a volume comes
/// back, so the index follows whatever changed the source. The first pass
/// after an upgrade backfills a store that predates the index the same way.
pub async fn index_text_soon(
	library: &Arc<crate::library::Library>,
	root: &Path,
	background: bool,
) {
	let volume_index = library.core_context().volume_index();
	if !volume_index
		.source_config_at(root)
		.is_some_and(|config| config.identify_content)
	{
		return;
	}
	let Some(store) = volume_index.store_for(root).await else {
		return;
	};
	let pending = match (store.text_index().await, text_filter(&store).await) {
		(Ok(index), Ok(filter)) => index.pending_count(&filter).await,
		(Err(error), _) | (_, Err(error)) => Err(sd_store::Error::Other(error.to_string())),
	};
	match pending {
		Ok(0) => return,
		Ok(_) => {}
		Err(error) => {
			tracing::warn!(root = %root.display(), %error, "could not count files waiting for the text index");
			return;
		}
	}
	if let Err(error) = library
		.jobs()
		.dispatch_with_priority(
			ContentTextJob::new(root.to_path_buf(), background),
			JobPriority::LOW,
			None,
		)
		.await
	{
		tracing::warn!(root = %root.display(), %error, "could not start the text index pass");
	}
}

/// What a text pass over the source at `root` reads: every extension an
/// extractor handles, under the source's deep scope.
pub async fn text_filter(store: &super::store::SourceStore) -> anyhow::Result<TextFilter> {
	Ok(TextFilter {
		extensions: TextKind::extensions()
			.into_iter()
			.map(String::from)
			.collect(),
		subtrees: store.deep_subtrees().await?,
	})
}
