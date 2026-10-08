//! Identifying the bytes behind a source's records.
//!
//! A record's identity is assigned when it is discovered and follows the file
//! through renames. Its *content's* identity is derived from the bytes, so two
//! machines that have never spoken compute the same id for the same file. That
//! is what turns "these are two files" into "this is one file in two places",
//! and it is the only evidence that survives crossing to another device: a path
//! is weak and an inode does not travel at all.
//!
//! Only the cheap tier runs here. A sampled hash over a few regions is enough to
//! group candidates, and the integrity hash that turns a candidate into a
//! certainty costs a full read of every byte on the drive. It belongs behind a
//! decision that needs it, which is the same rule the ladder in
//! `sd_store::content` exists to keep: never delete one copy of two on the
//! strength of a guess.
//!
//! The pass also decides the content's kind, since it is the one place that
//! opens every file. The file extension answers for a built-in kind. A file
//! an extension kind claims with magic patterns, or one no type claims, has
//! its first bytes checked against the extension kinds' patterns; the
//! built-in patterns stay unused as before. Both the kind and, for an
//! extension kind, its name land on the content row.

use crate::{
	domain::{content_identity::ContentHashGenerator, ContentKind},
	filetype::{registry::MAX_MAGIC_BYTES, FileTypeRegistry},
	infra::job::{generic_progress::GenericProgress, prelude::*, types::JobPriority},
	ops::indexing::SourceStore,
};
use futures::StreamExt;
use sd_store::ContentIdentity;
use serde::{Deserialize, Serialize};
use std::{
	path::{Path, PathBuf},
	sync::Arc,
};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

/// Files claimed from the store per pass. Large enough that the queue round
/// trip disappears against the reads, small enough that an interrupt lands
/// promptly.
const BATCH_SIZE: usize = 256;

/// Files hashed at once. A sampled hash is four short reads and a seek, so the
/// limit is the drive's appetite for concurrent seeks rather than the CPU.
const CONCURRENCY: usize = 8;

/// Hashes the files a source holds that have no content identity yet.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ContentIdentityJob {
	/// The source's root, which is how its store is found.
	root: PathBuf,
	/// Dispatched by the watcher's hashing nudge or the launch pass rather
	/// than a user action. The default keeps resumed pre-flag state valid.
	#[serde(default)]
	background: bool,
}

impl ContentIdentityJob {
	pub fn new(root: PathBuf) -> Self {
		Self {
			root,
			background: false,
		}
	}

	/// A pass nobody asked for by hand: the launch sweep or a watcher nudge.
	pub fn background(root: PathBuf) -> Self {
		Self {
			root,
			background: true,
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentIdentityOutput {
	pub identified: u64,
	pub unreadable: u64,
}

impl From<ContentIdentityOutput> for JobOutput {
	fn from(output: ContentIdentityOutput) -> Self {
		JobOutput::custom(output)
	}
}

impl Job for ContentIdentityJob {
	const NAME: &'static str = "content_identity";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Identify file contents");
}

impl crate::infra::job::traits::DynJob for ContentIdentityJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	/// One hashing pass per source at a time. The pass claims its work from
	/// the store, so a second dispatch would only duplicate reads.
	fn dedup_key(&self) -> Option<String> {
		Some(self.root.display().to_string())
	}

	/// Background passes keep their job row for the record but stay off the
	/// event bus: the watcher nudges one per dirty root every thirty seconds
	/// on a busy drive, and a launch dispatches one per source, so announcing
	/// each start and completion turns routine upkeep into a stream of
	/// finished-job notifications.
	fn should_emit_events(&self) -> bool {
		!self.background
	}
}

#[async_trait::async_trait]
impl JobHandler for ContentIdentityJob {
	type Output = ContentIdentityOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		// A file read through an unmounted drive's mount point fails and is
		// recorded as unreadable, which takes it out of the pending set for
		// good; refusing the whole pass keeps those files pending for when
		// the drive returns.
		if let Some(reason) = ctx
			.library()
			.core_context()
			.volume_index()
			.dispatch_refusal(&self.root)
		{
			return Err(JobError::execution(reason));
		}
		let Some(store) = ctx
			.library()
			.core_context()
			.volume_index()
			.store_for(&self.root)
			.await
		else {
			// Nothing is kept from this root, so there are no records to hang
			// a content identity on. Not a failure: it is what a drive that is
			// mapped and not in the library looks like.
			ctx.log(format!(
				"No source store at {}; nothing to identify",
				self.root.display()
			));
			return Ok(ContentIdentityOutput {
				identified: 0,
				unreadable: 0,
			});
		};

		// The walk that found these files may still be committing them.
		store.flush().await.map_err(|e| e.to_string())?;

		let outstanding = store
			.files_needing_content_count()
			.await
			.map_err(|e| e.to_string())?;
		ctx.log(format!("{outstanding} files to identify"));

		let label = source_label(&self.root);

		// Say what the job is doing before the first byte is read. A card
		// that shows a type name and a bare percentage is not progress.
		if outstanding > 0 {
			ctx.progress(Progress::generic(GenericProgress::new(
				0.0,
				"Identifying",
				format!("{label} — 0 of {outstanding} files"),
			)));
		}

		// Progress moves within a claim, not only between claims: a claim of
		// large files hashes for a while, and a bar that only advances per
		// claim reads as stuck.
		const PROGRESS_CHUNK: usize = 32;

		let mut identified = 0u64;
		let mut unreadable = 0u64;
		let registry = FileTypeRegistry::current();

		loop {
			ctx.check_interrupt().await?;

			let batch = store
				.files_needing_content(BATCH_SIZE)
				.await
				.map_err(|e| e.to_string())?;
			if batch.is_empty() {
				break;
			}

			for chunk in batch.chunks(PROGRESS_CHUNK) {
				ctx.check_interrupt().await?;
				let (identities, failures) = hash_batch(chunk.to_vec(), &registry).await;

				// Failures leave the pending set with their reason recorded,
				// so the loop always advances and the store can say
				// afterwards which files have no identity and why.
				unreadable += failures.len() as u64;
				identified += identities.len() as u64;
				store.identified(identities).await;
				store.content_unreadable(failures).await;

				let done = identified + unreadable;
				ctx.progress(Progress::generic(GenericProgress::new(
					if outstanding > 0 {
						(done as f32 / outstanding as f32).min(1.0)
					} else {
						1.0
					},
					"Identifying",
					format!("{label} — {done} of {outstanding} files"),
				)));
			}

			// Wait for this claim to land before taking the next. The pending
			// query reads the database, and a query that outraces the writer
			// hands back the same files forever.
			store.flush().await.map_err(|e| e.to_string())?;
		}

		// Content identities are queued behind the same writer as everything
		// else, so the job is not done until they have landed.
		store.flush().await.map_err(|e| e.to_string())?;

		ctx.log(format!(
			"Identified {identified} files, {unreadable} unreadable"
		));

		Ok(ContentIdentityOutput {
			identified,
			unreadable,
		})
	}
}

/// The name a person knows the source by, for progress that says what it is
/// working on rather than only how far along it is.
pub(crate) fn source_label(root: &std::path::Path) -> String {
	root.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_else(|| root.display().to_string())
}

/// Hash a batch, several files at a time, dropping the ones that cannot be read.
///
/// A file that vanished between being listed and being opened is the ordinary
/// case, not an error: the walk that recorded it ran earlier, and the next one
/// will remove it. Either way the failure is returned with its reason, so the
/// store can take the file out of the pending set instead of handing it back
/// on every pass.
async fn hash_batch(
	batch: Vec<(Uuid, PathBuf, u64)>,
	registry: &FileTypeRegistry,
) -> (Vec<(Uuid, ContentIdentity)>, Vec<(Uuid, String)>) {
	let results: Vec<_> = futures::stream::iter(batch)
		.map(|(uuid, path, size)| async move {
			match ContentHashGenerator::generate_content_hash(&path).await {
				Ok(hash) => {
					let (kind, kind_name) = identify_kind(&path, registry).await;
					Ok((
						uuid,
						ContentIdentity {
							sampled_hash: Some(hash),
							integrity_hash: None,
							size: Some(size as i64),
							kind,
							kind_name,
						},
					))
				}
				Err(error) => {
					tracing::debug!(path = %path.display(), %error, "could not hash");
					Err((uuid, error.to_string()))
				}
			}
		})
		.buffer_unordered(CONCURRENCY)
		.collect()
		.await;

	let mut identities = Vec::new();
	let mut failures = Vec::new();
	for result in results {
		match result {
			Ok(identity) => identities.push(identity),
			Err(failure) => failures.push(failure),
		}
	}
	(identities, failures)
}

/// The kind a file's name and, when an extension kind's patterns are in
/// play, its first bytes decide: the `ContentKind` discriminant to store,
/// and the extension kind's id when one claimed the file.
///
/// Bytes are read only when the registry has patterns to check for this
/// extension; a plain `.jpg` costs no second open. A header that cannot be
/// read keeps the extension result, since the hash just succeeded and the
/// file was there a moment ago.
async fn identify_kind(path: &Path, registry: &FileTypeRegistry) -> (Option<i64>, Option<String>) {
	let by_extension = registry.type_by_extension(path);
	let extension = path.extension().and_then(|e| e.to_str());
	let candidates = registry.magic_candidates(extension);
	let file_type = if candidates.is_empty() {
		by_extension
	} else {
		let mut header = vec![0u8; MAX_MAGIC_BYTES];
		let read = match tokio::fs::File::open(path).await {
			Ok(mut file) => file.read(&mut header).await.unwrap_or(0),
			Err(_) => 0,
		};
		header.truncate(read);
		registry.resolve_by_magic(by_extension, &candidates, &header)
	};
	let Some(file_type) = file_type else {
		return (None, None);
	};
	let kind = (file_type.category != ContentKind::Unknown).then_some(file_type.category as i64);
	(kind, file_type.kind_name.clone())
}

/// Queue the hashing of every source on this machine, behind whatever else is
/// running.
///
/// Last of the three passes a launch dispatches. The library's own walk goes
/// first because it is what someone chose to keep, the drive map second because
/// the analyser needs the whole picture, and this last because nothing on screen
/// is waiting for it.
pub async fn identify_every_source(
	library: &Arc<crate::library::Library>,
	context: &Arc<crate::context::CoreContext>,
) {
	for source in context.volume_index().sources_of(library.id()) {
		if !source.attached {
			continue;
		}
		if let Some(reason) = context.volume_index().dispatch_refusal(&source.root) {
			tracing::warn!(source = %source.id, %reason, "not identifying the source");
			continue;
		}

		let job = ContentIdentityJob::background(source.root.clone());
		match library
			.jobs()
			.dispatch_with_priority(job, JobPriority::LOW, None)
			.await
		{
			Ok(handle) => tracing::info!(
				"Identifying contents of {} in the background as job {}",
				source.root.display(),
				handle.id()
			),
			Err(error) => tracing::warn!(
				"Could not start content identification for {}: {error}",
				source.root.display()
			),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn an_unreadable_file_does_not_stop_the_batch() {
		let dir = tempfile::tempdir().unwrap();
		let real = dir.path().join("real.bin");
		std::fs::write(&real, vec![7u8; 4096]).unwrap();

		let (identities, failures) = hash_batch(
			vec![
				(Uuid::now_v7(), dir.path().join("gone.bin"), 10),
				(Uuid::now_v7(), real, 4096),
			],
			FileTypeRegistry::builtin(),
		)
		.await;

		assert_eq!(identities.len(), 1);
		assert!(identities[0].1.sampled_hash.is_some());
		assert_eq!(failures.len(), 1, "the missing file failed with a reason");
	}

	#[tokio::test]
	async fn identical_bytes_hash_identically() {
		let dir = tempfile::tempdir().unwrap();
		let one = dir.path().join("one.bin");
		let two = dir.path().join("two.bin");
		std::fs::write(&one, vec![3u8; 8192]).unwrap();
		std::fs::write(&two, vec![3u8; 8192]).unwrap();

		let (identities, _) = hash_batch(
			vec![(Uuid::now_v7(), one, 8192), (Uuid::now_v7(), two, 8192)],
			FileTypeRegistry::builtin(),
		)
		.await;

		assert_eq!(identities.len(), 2);
		assert_eq!(
			identities[0].1.sampled_hash, identities[1].1.sampled_hash,
			"two copies of the same bytes are one content row"
		);
	}

	#[tokio::test]
	async fn a_built_in_kind_is_stored_and_an_extension_kind_reads_the_header() {
		use crate::filetype::{ExtensionKind, MagicPatternSpec};
		let dir = tempfile::tempdir().unwrap();
		let photo = dir.path().join("a.jpg");
		std::fs::write(&photo, vec![0xFF; 4096]).unwrap();
		let nothing = dir.path().join("b.zzzkind");
		std::fs::write(&nothing, b"ZZKIND rest of file".repeat(300)).unwrap();
		let other = dir.path().join("c.zzzkind");
		std::fs::write(&other, vec![0u8; 4096]).unwrap();

		let builtin = FileTypeRegistry::builtin();
		assert_eq!(identify_kind(&photo, builtin).await, (Some(1), None));
		assert_eq!(identify_kind(&nothing, builtin).await, (None, None));

		let layered = FileTypeRegistry::with_extension_kinds(&[(
			"ext".into(),
			vec![ExtensionKind {
				name: "zz".into(),
				display_name: None,
				parent: ContentKind::Text,
				extensions: vec!["zzzkind".into()],
				mime_types: Vec::new(),
				magic: vec![MagicPatternSpec {
					pattern: "5A 5A 4B 49 4E 44".into(),
					offset: 0,
				}],
				preview: None,
			}],
		)]);
		assert_eq!(
			identify_kind(&nothing, &layered).await,
			(Some(ContentKind::Text as i64), Some("ext:zz".into()))
		);
		assert_eq!(
			identify_kind(&other, &layered).await,
			(Some(ContentKind::Text as i64), Some("ext:zz".into())),
			"no magic match keeps the extension result"
		);
		assert_eq!(identify_kind(&photo, &layered).await, (Some(1), None));
	}
}
