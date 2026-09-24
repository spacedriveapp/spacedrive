//! Deleting from A the files a comparison with B sorts into one set.
//!
//! The comparison is the job's input, so the job derives the set itself as it
//! runs instead of trusting a list a client computed earlier: it drains the
//! matcher the compare query pages, in the same order, and checkpoints the
//! cursor after each batch, so a restart resumes where it stopped.
//!
//! A file in both is a copy A can lose only if B's copy holds the same bytes,
//! and the comparison matched them on a sampled hash, or on size and
//! modification time where a side is unhashed. So before removing one the job
//! reads whichever side has not been read in full and compares integrity
//! hashes, the rule the content ladder exists for: never delete one copy of
//! two on the strength of a guess. What a read learns is written back to the
//! store, so it is paid once. A pair whose bytes differ, or whose copy in B
//! is gone by the time its turn comes, stays in A and is reported.

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Instant};

use futures::StreamExt;
use sd_store::ContentIdentity;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::{content_identity::ContentHashGenerator, SdPath},
	infra::{
		job::{generic_progress::GenericProgress, prelude::*},
		query::QueryError,
	},
	ops::paths::compare::{
		open_folders, CompareBy, CompareCursor, CompareSet, Comparison, Keyed, Matcher, Side,
		Sorted,
	},
};

use super::job::{DeleteMode, DeleteOutput, SkipReason, Tally};
use super::routing::DeleteStrategyRouter;

/// Files settled and removed between checkpoints.
const BATCH: usize = 256;

/// Files read in full at once. Full reads want the drive to themselves; two
/// keeps a spindle busy without turning throughput into seeks.
pub(super) const READS: usize = 2;

pub(super) async fn delete(
	ctx: &JobContext<'_>,
	comparison: &Comparison,
	mode: DeleteMode,
	started_at: Instant,
) -> JobResult<DeleteOutput> {
	let context = ctx.library().core_context().clone();
	let show = comparison.show;
	let after: Option<CompareCursor> = ctx.load_state().await?;
	if after.is_some() {
		ctx.log("Resuming from the last checkpoint");
	}

	// The index alone says how much there is, so progress has a total and
	// the log says what the job is about to do.
	ctx.progress(Progress::Indeterminate("Counting".to_string()));
	let (total, total_bytes) = count(&context, comparison, after.as_ref()).await?;
	ctx.log(format!(
		"{total} files ({total_bytes} bytes) {} to {}",
		set_name(show),
		mode.label()
	));

	let (mut a, mut b) = open_folders(&context, comparison, after.as_ref())
		.await
		.map_err(failed)?;
	let mut matcher = match comparison.by {
		CompareBy::Path => Matcher::by_path(&mut a, &mut b),
		CompareBy::Content => Matcher::by_content(&mut a, &b, Side::A),
	};

	let mut tally = Tally::default();
	let mut handled = 0u64;
	loop {
		ctx.check_interrupt().await?;
		let mut batch = Vec::with_capacity(BATCH);
		while batch.len() < BATCH {
			match matcher.next().await.map_err(failed)? {
				Some(sorted) if sorted.set == show => batch.push(sorted),
				Some(_) => continue,
				None => break,
			}
		}
		if batch.is_empty() {
			break;
		}
		let cursor = batch.last().and_then(Sorted::key).cloned();
		handled += batch.len() as u64;

		let skipped_before = tally.skipped.len();
		let (removable, learned) = if show == CompareSet::Both {
			settle(&matcher, comparison.by, batch, &mut tally).await?
		} else {
			(
				batch.into_iter().filter_map(|sorted| sorted.a).collect(),
				Vec::new(),
			)
		};
		for skip in &tally.skipped[skipped_before..] {
			ctx.log(format!("Left {}: {}", skip.path.display(), skip.reason));
		}

		let paths: Vec<SdPath> = removable
			.iter()
			.map(|file| SdPath::local(file.path.clone()))
			.collect();
		if !paths.is_empty() {
			let strategy =
				DeleteStrategyRouter::select_strategy(&paths, ctx.volume_manager().as_deref())
					.await;
			let results = strategy
				.execute(ctx, &paths, mode.clone())
				.await
				.map_err(|e| JobError::execution(format!("Strategy execution failed: {e}")))?;
			tally.record(results);
		}
		record_learned(ctx, &context, learned).await;

		ctx.progress(Progress::generic(
			GenericProgress::new(
				(handled as f32 / total.max(1) as f32).min(1.0),
				"Deleting",
				format!("{handled} of {total} files"),
			)
			.with_completion(handled, total)
			.with_bytes(tally.bytes, total_bytes)
			.with_errors(tally.failed.len() as u64, tally.skipped.len() as u64),
		));
		if let Some((directory, name)) = cursor {
			ctx.checkpoint_with_state(&CompareCursor { directory, name })
				.await?;
		}
	}

	ctx.log(format!(
		"Delete operation completed: {} deleted, {} skipped, {} failed",
		tally.deleted,
		tally.skipped.len(),
		tally.failed.len()
	));
	Ok(tally.into_output(mode, started_at))
}

/// How many of A's files the set holds from `after` on, and their bytes.
async fn count(
	context: &CoreContext,
	comparison: &Comparison,
	after: Option<&CompareCursor>,
) -> JobResult<(u64, u64)> {
	let (mut a, mut b) = open_folders(context, comparison, after)
		.await
		.map_err(failed)?;
	let mut matcher = match comparison.by {
		CompareBy::Path => Matcher::by_path(&mut a, &mut b),
		CompareBy::Content => Matcher::by_content(&mut a, &b, Side::A),
	};
	let (mut files, mut bytes) = (0, 0);
	while let Some(sorted) = matcher.next().await.map_err(failed)? {
		if sorted.set == comparison.show {
			files += 1;
			bytes += sorted
				.a
				.as_ref()
				.and_then(|file| file.entry.size)
				.unwrap_or(0)
				.max(0) as u64;
		}
	}
	Ok((files, bytes))
}

/// Which of a batch of A's files in both may go: those whose copy in B holds
/// the same bytes, read in full. The rest are skipped with why. Returns them
/// with what the reads learned, for the stores.
async fn settle(
	matcher: &Matcher<'_>,
	by: CompareBy,
	batch: Vec<Sorted>,
	tally: &mut Tally,
) -> JobResult<(Vec<Keyed>, Vec<Learned>)> {
	let mut learned = Vec::new();

	// A's files, and by path the copy at each one's place in B.
	let mut files = Vec::with_capacity(batch.len());
	let mut copies = Vec::with_capacity(batch.len());
	for sorted in batch {
		if let Some(file) = sorted.a {
			files.push(file);
			copies.push(sorted.b);
		}
	}

	// The copies in B, settled once each: by path the file at the same
	// place, by content one holder per content, which stands for every file
	// in B holding it.
	let mut theirs: Vec<Option<Result<String, String>>> = Vec::with_capacity(files.len());
	match by {
		CompareBy::Path => {
			let mut settled = futures::stream::iter(copies)
				.map(|copy| async move {
					match copy {
						Some(copy) => Some(integrity(&copy).await),
						None => None,
					}
				})
				.buffered(READS);
			while let Some(copy) = settled.next().await {
				theirs.push(copy.map(|(hash, learnt)| {
					learned.extend(learnt);
					hash
				}));
			}
		}
		CompareBy::Content => {
			let contents: Vec<String> = files
				.iter()
				.filter_map(|file| file.entry.sampled_hash.clone())
				.collect();
			let holders = matcher.holders(&contents).await.map_err(failed)?;
			let mut held: HashMap<String, Result<String, String>> = HashMap::new();
			let mut settled = futures::stream::iter(holders)
				.map(|holder| async move {
					let (hash, learnt) = integrity(&holder).await;
					(holder.entry.sampled_hash, hash, learnt)
				})
				.buffered(READS);
			while let Some((sampled, hash, learnt)) = settled.next().await {
				learned.extend(learnt);
				if let Some(sampled) = sampled {
					held.insert(sampled, hash);
				}
			}
			theirs.extend(files.iter().map(|file| {
				file.entry
					.sampled_hash
					.as_ref()
					.and_then(|sampled| held.get(sampled).cloned())
			}));
		}
	}

	let mut files = futures::stream::iter(files)
		.map(|file| async move {
			let (hash, learnt) = integrity(&file).await;
			(file, hash, learnt)
		})
		.buffered(READS);
	let mut removable = Vec::new();
	let mut theirs = theirs.into_iter();
	while let Some((file, hash, learnt)) = files.next().await {
		let verdict = match (theirs.next().flatten(), hash) {
			(None, _) => Err(SkipReason::NoCopy),
			(Some(Ok(theirs)), Ok(ours)) if theirs == ours => Ok(()),
			(Some(Ok(_)), Ok(_)) => Err(SkipReason::Differs),
			(Some(Err(error)), _) | (_, Err(error)) => Err(SkipReason::Unreadable(error)),
		};
		match verdict {
			Ok(()) => removable.push(file),
			Err(reason) => {
				// The file stays, so what was read of it is worth keeping.
				learned.extend(learnt);
				tally.skip(file.path.clone(), reason);
			}
		}
	}
	Ok((removable, learned))
}

/// A file's integrity hash: the store's where it has read the file in full,
/// else read now, with the identity the store should learn.
pub(super) async fn integrity(file: &Keyed) -> (Result<String, String>, Option<Learned>) {
	if let Some(hash) = &file.entry.integrity_hash {
		return (Ok(hash.clone()), None);
	}
	let integrity = match ContentHashGenerator::generate_integrity_hash(&file.path).await {
		Ok(hash) => hash,
		Err(error) => return (Err(error.to_string()), None),
	};
	// The content row is keyed by the sampled hash, so the verdict carries
	// it, computed here for a file the sampled tier has not reached.
	let sampled = match &file.entry.sampled_hash {
		Some(hash) => hash.clone(),
		None => match ContentHashGenerator::generate_content_hash(&file.path).await {
			Ok(hash) => hash,
			Err(error) => return (Err(error.to_string()), None),
		},
	};
	let learned = Learned {
		root: file.root.clone(),
		uuid: file.entry.uuid,
		identity: ContentIdentity {
			sampled_hash: Some(sampled),
			integrity_hash: Some(integrity.clone()),
			size: file.entry.size,
			kind: None,
		},
	};
	(Ok(integrity), Some(learned))
}

/// What a read in full learned about a file, for its source's store.
pub(super) struct Learned {
	root: Arc<PathBuf>,
	uuid: Uuid,
	identity: ContentIdentity,
}

/// Write what the batch's reads learned to each source's store, so the next
/// operation over these files reads nothing.
pub(super) async fn record_learned(
	ctx: &JobContext<'_>,
	context: &CoreContext,
	learned: Vec<Learned>,
) {
	let mut by_root: HashMap<Arc<PathBuf>, Vec<(Uuid, ContentIdentity)>> = HashMap::new();
	for learnt in learned {
		by_root
			.entry(learnt.root)
			.or_default()
			.push((learnt.uuid, learnt.identity));
	}
	for (root, identities) in by_root {
		let Some(store) = context.volume_index().store_for(&root).await else {
			continue;
		};
		store.identified(identities).await;
		if let Err(error) = store.flush().await {
			ctx.add_warning(format!(
				"Could not record what was read of {}: {error}",
				root.display()
			));
		}
	}
}

fn set_name(set: CompareSet) -> &'static str {
	match set {
		CompareSet::OnlyA => "only in A",
		CompareSet::OnlyB => "only in B",
		CompareSet::Both => "in both",
		CompareSet::Different => "different in B",
	}
}

fn failed(error: QueryError) -> JobError {
	JobError::execution(error.to_string())
}
