//! Turning candidates into certainties.
//!
//! The sampled tier says two files are probably the same bytes; the rule the
//! ladder exists to keep is that nobody deletes one copy of two on the
//! strength of probably. This job reads every byte of exactly the files whose
//! content is shared, writes the integrity hash onto the same content row,
//! and the row's uuid upgrades to the integrity-derived id as it lands.
//!
//! The work queue is the store, not a path and not a result set. Claiming
//! "shared content, integrity still null" from the database means the job
//! stays current as the sampled tier keeps landing behind it, resumes for
//! free, and never reads a byte of a file that has no duplicate.

use crate::{
	domain::content_identity::ContentHashGenerator,
	infra::job::{generic_progress::GenericProgress, prelude::*},
};
use futures::StreamExt;
use sd_store::ContentIdentity;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Files claimed per pass. Smaller than the sampled tier's batch because each
/// item here is a full read, often of something enormous.
const BATCH_SIZE: usize = 64;

/// Files read at once. Full sequential reads want the drive to themselves;
/// two keeps a spindle busy without turning throughput into seeks.
const CONCURRENCY: usize = 2;

/// Reads every byte of a source's shared-content files.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct VerifyContentJob {
	/// The source's root, which is how its store is found.
	root: PathBuf,
}

impl VerifyContentJob {
	pub fn new(root: PathBuf) -> Self {
		Self { root }
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyContentOutput {
	pub verified: u64,
	pub unreadable: u64,
	pub bytes_read: u64,
}

impl From<VerifyContentOutput> for JobOutput {
	fn from(output: VerifyContentOutput) -> Self {
		JobOutput::custom(output)
	}
}

impl Job for VerifyContentJob {
	const NAME: &'static str = "verify_content";
	const RESUMABLE: bool = true;
	const DESCRIPTION: Option<&'static str> = Some("Verify duplicate file contents");
}

impl crate::infra::job::traits::DynJob for VerifyContentJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	/// One verification pass per source at a time; the work queue is shared.
	fn dedup_key(&self) -> Option<String> {
		Some(self.root.display().to_string())
	}
}

#[async_trait::async_trait]
impl JobHandler for VerifyContentJob {
	type Output = VerifyContentOutput;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let Some(store) = ctx
			.library()
			.core_context()
			.volume_index()
			.store_for(&self.root)
			.await
		else {
			ctx.log(format!(
				"No source store at {}; nothing to verify",
				self.root.display()
			));
			return Ok(VerifyContentOutput {
				verified: 0,
				unreadable: 0,
				bytes_read: 0,
			});
		};

		// The sampled tier may still be landing; verify what has landed.
		store.flush().await.map_err(|e| e.to_string())?;

		let outstanding = store
			.files_needing_verification_count()
			.await
			.map_err(|e| e.to_string())?;
		ctx.log(format!("{outstanding} shared-content files to verify"));

		let label = crate::ops::indexing::content_identity::source_label(&self.root);

		// Say what the job is doing before the first byte is read. A card
		// that shows a type name and a bare percentage is not progress.
		if outstanding > 0 {
			ctx.progress(Progress::generic(GenericProgress::new(
				0.0,
				"Verifying",
				format!("{label} — 0 of {outstanding} files"),
			)));
		}

		// Verification reads every byte, so a claim of large files can run
		// for minutes; progress advances within the claim, not only after it.
		const PROGRESS_CHUNK: usize = 8;

		let mut verified = 0u64;
		let mut unreadable = 0u64;
		let mut bytes_read = 0u64;

		loop {
			ctx.check_interrupt().await?;

			let batch = store
				.files_needing_verification(BATCH_SIZE)
				.await
				.map_err(|e| e.to_string())?;
			if batch.is_empty() {
				break;
			}

			for chunk in batch.chunks(PROGRESS_CHUNK) {
				ctx.check_interrupt().await?;
				let (identities, failures, chunk_bytes) = verify_batch(chunk.to_vec()).await;

				unreadable += failures.len() as u64;
				verified += identities.len() as u64;
				bytes_read += chunk_bytes;
				store.identified(identities).await;
				store.content_unreadable(failures).await;

				let done = verified + unreadable;
				ctx.progress(Progress::generic(GenericProgress::new(
					if outstanding > 0 {
						(done as f32 / outstanding as f32).min(1.0)
					} else {
						1.0
					},
					"Verifying",
					format!("{label} — {done} of {outstanding} files"),
				)));
			}

			// Wait for this claim to land before taking the next, so the
			// pending query sees it and the loop always advances.
			store.flush().await.map_err(|e| e.to_string())?;
		}

		// Verification results are queued behind the same writer as everything
		// else; the job succeeds only once they have durably landed.
		store.flush().await.map_err(|e| e.to_string())?;

		ctx.log(format!(
			"Verified {verified} files ({bytes_read} bytes read), {unreadable} unreadable"
		));

		Ok(VerifyContentOutput {
			verified,
			unreadable,
			bytes_read,
		})
	}
}

/// Read every byte of a batch, a couple of files at a time.
///
/// The verdict carries the sampled hash the content row is keyed by, so the
/// upsert upgrades that row rather than minting a second one, and the two
/// tiers stay on one identity.
async fn verify_batch(
	batch: Vec<(Uuid, PathBuf, u64, Option<String>)>,
) -> (Vec<(Uuid, ContentIdentity)>, Vec<(Uuid, String)>, u64) {
	let results: Vec<_> = futures::stream::iter(batch)
		.map(|(uuid, path, size, sampled_hash)| async move {
			match ContentHashGenerator::generate_integrity_hash(&path).await {
				Ok(hash) => Ok((
					uuid,
					ContentIdentity {
						sampled_hash,
						integrity_hash: Some(hash),
						size: Some(size as i64),
						kind: None,
					},
					size,
				)),
				Err(error) => {
					tracing::debug!(path = %path.display(), %error, "could not verify");
					Err((uuid, error.to_string()))
				}
			}
		})
		.buffer_unordered(CONCURRENCY)
		.collect()
		.await;

	let mut identities = Vec::new();
	let mut failures = Vec::new();
	let mut bytes = 0u64;
	for result in results {
		match result {
			Ok((uuid, identity, size)) => {
				identities.push((uuid, identity));
				bytes += size;
			}
			Err(failure) => failures.push(failure),
		}
	}
	(identities, failures, bytes)
}
