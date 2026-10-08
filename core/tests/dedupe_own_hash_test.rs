//! Dedupe may remove a copy only on the strength of that copy's own bytes.
//!
//! Before schema version 1 every record with one sampled hash shared one
//! content row, so an integrity hash read for one file stood for every file
//! that merely sampled alike, and dedupe removed a file whose bytes had never
//! been read in full as a copy of another. This test sets up exactly that
//! store state and checks the file stays.

use sd_core::{
	domain::{content_identity::ContentHashGenerator, SdPath},
	infra::{action::LibraryAction, job::types::JobStatus},
	ops::{
		files::delete::{
			action::FileDeleteAction,
			input::{DeleteTargets, Duplicates, FileDeleteInput, Keep},
		},
		sources::track::{TrackSourceAction, TrackSourceInput},
	},
	Core,
};
use sd_store::ContentIdentity;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::{sleep, Instant};
use uuid::Uuid;

#[tokio::test]
async fn dedupe_keeps_a_file_whose_own_bytes_were_never_read_in_full(
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let temp_dir = TempDir::new()?;
	let core = Core::new(temp_dir.path().join("core")).await?;
	let library = core
		.libraries
		.create_library("Dedupe", None, core.context.clone())
		.await?;

	// Same size, different bytes: what a VM disk and its backup look like
	// after the disk changed somewhere the sampled hash does not read.
	let source_dir = temp_dir.path().join("source");
	tokio::fs::create_dir_all(&source_dir).await?;
	let verified = source_dir.join("a.bin");
	let lookalike = source_dir.join("b.bin");
	tokio::fs::write(&verified, vec![1u8; 4096]).await?;
	tokio::fs::write(&lookalike, vec![2u8; 4096]).await?;

	let tracked = TrackSourceAction::from_input(TrackSourceInput {
		path: source_dir.clone(),
		name: None,
		overrides: Default::default(),
	})?
	.execute(library.clone(), core.context.clone())
	.await?;
	let store = core
		.context
		.volume_index()
		.store_for(&tracked.root)
		.await
		.ok_or("the tracked source has no store")?;

	let deadline = Instant::now() + Duration::from_secs(30);
	loop {
		if store.files_needing_content_count().await? == 0
			&& store.counts().await.map_or(0, |counts| counts.contents) == 2
		{
			break;
		}
		assert!(
			Instant::now() < deadline,
			"the walk and hash did not finish"
		);
		sleep(Duration::from_millis(50)).await;
	}

	// Force the store into the state the plan names: both files believed to
	// sample alike, a.bin read in full, b.bin never read in full.
	let a = store
		.db()
		.resolve_path("a.bin")
		.await?
		.ok_or("a.bin has no record")?;
	let b = store
		.db()
		.resolve_path("b.bin")
		.await?
		.ok_or("b.bin has no record")?;
	let integrity_of_a = ContentHashGenerator::generate_integrity_hash(&verified).await?;
	let shared_sample = "sampled-alike".to_string();
	store
		.db()
		.set_content_identity(
			a,
			&ContentIdentity {
				sampled_hash: Some(shared_sample.clone()),
				integrity_hash: Some(integrity_of_a.clone()),
				size: Some(4096),
				kind: None,
				kind_name: None,
			},
		)
		.await?;
	store
		.db()
		.set_content_identity(
			b,
			&ContentIdentity {
				sampled_hash: Some(shared_sample.clone()),
				integrity_hash: None,
				size: Some(4096),
				kind: None,
				kind_name: None,
			},
		)
		.await?;

	// The store never lends a.bin's hash to b.bin.
	let (b_sampled, b_integrity): (Option<String>, Option<String>) = sqlx::query_as(
		"SELECT c.sampled_hash, c.integrity_hash FROM record r JOIN content c ON c.id = r.content_id
		 WHERE r.uuid = ?",
	)
	.bind(b)
	.fetch_one(store.db().pool())
	.await?;
	assert_eq!(b_sampled.as_deref(), Some(shared_sample.as_str()));
	assert_eq!(
		b_integrity, None,
		"b.bin's own bytes were never read in full"
	);

	let receipt = FileDeleteAction::from_input(FileDeleteInput {
		targets: DeleteTargets::Duplicates {
			duplicates: Duplicates {
				scope: Some(SdPath::local(source_dir.clone())),
				keep: Keep::First,
				min_size: Some(0),
			},
		},
		permanent: true,
		recursive: false,
	})?
	.execute(library.clone(), core.context.clone())
	.await?;

	let deadline = Instant::now() + Duration::from_secs(30);
	let info = loop {
		let info = library.jobs().get_job_info(receipt.id.0).await?;
		if let Some(info) = info.filter(|info| {
			matches!(
				info.status,
				JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
			)
		}) {
			break info;
		}
		assert!(Instant::now() < deadline, "the dedupe job did not finish");
		sleep(Duration::from_millis(50)).await;
	};
	assert_eq!(
		info.status,
		JobStatus::Completed,
		"{:?}",
		info.error_message
	);

	assert!(verified.exists(), "the keeper stays");
	assert!(
		lookalike.exists(),
		"a file whose own integrity hash was never computed is not deleted as a copy"
	);

	// Reading b.bin in full to settle the pair gave it a confirmed row of
	// its own, holding its own hash.
	let integrity_of_b = ContentHashGenerator::generate_integrity_hash(&lookalike).await?;
	let (stored_b, rows_for_sample): (Option<String>, i64) = sqlx::query_as(
		"SELECT c.integrity_hash,
		        (SELECT COUNT(*) FROM content WHERE sampled_hash = c.sampled_hash)
		 FROM record r JOIN content c ON c.id = r.content_id WHERE r.uuid = ?",
	)
	.bind(b)
	.fetch_one(store.db().pool())
	.await?;
	assert_eq!(stored_b.as_deref(), Some(integrity_of_b.as_str()));
	assert_ne!(integrity_of_a, integrity_of_b);
	assert_eq!(rows_for_sample, 2, "one confirmed row per set of bytes");

	let _: Uuid = a;
	core.shutdown().await?;
	Ok(())
}
