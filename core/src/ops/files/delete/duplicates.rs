//! Removing the surplus copies of duplicated content.
//!
//! The copies are derived from the index as the job runs. Keeping the first
//! copy streams the scope in the order the compare query pages it, and the
//! first file holding each duplicated content is the one that stays; every
//! later holder is read in full, as is the keeper once, and removed only
//! when the integrity hashes agree, the rule the comparison delete follows.
//! Chosen keepers are looked up in their stores, and every other holder of
//! their content in the scope is settled the same way. What a read learns is
//! written back to the store. The stream's cursor is checkpointed after each
//! batch; a resumed job rebuilds the keepers before the cursor from the index
//! alone, which by then holds what their reads learned.

use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	sync::Arc,
	time::Instant,
};

use futures::StreamExt;
use sd_store::FsEntry;

use super::{
	compared::{integrity, record_learned, Learned, READS},
	input::{Duplicates, Keep},
	job::{DeleteMode, DeleteOutput, SkipReason, Tally},
	routing::DeleteStrategyRouter,
};
use crate::{
	context::CoreContext,
	domain::SdPath,
	infra::{
		job::{generic_progress::GenericProgress, prelude::*},
		query::QueryError,
	},
	ops::{
		indexing::VolumeIndex,
		paths::{
			compare::{CompareCursor, Folder, Key, Keyed},
			reach::{every_store, stores_beneath, stores_beneath_in, Reach},
		},
	},
	volume::VolumeManager,
};

/// Copies settled between checkpoints.
const BATCH: usize = 256;

pub(super) async fn delete(
	ctx: &JobContext<'_>,
	duplicates: &Duplicates,
	mode: DeleteMode,
	started_at: Instant,
) -> JobResult<DeleteOutput> {
	let context = ctx.library().core_context().clone();
	match &duplicates.keep {
		Keep::First => keep_first(ctx, &context, duplicates, mode, started_at).await,
		Keep::These { paths } => {
			keep_these(ctx, &context, duplicates, paths, mode, started_at).await
		}
	}
}

/// The copy of a content that stays, and its integrity hash once known.
struct Keeper {
	file: Keyed,
	integrity: Option<Result<String, String>>,
}

impl Keeper {
	fn new(file: Keyed) -> Self {
		Self {
			file,
			integrity: None,
		}
	}

	/// The keeper's integrity hash: the store's, or read now, once.
	async fn integrity(&mut self, learned: &mut Vec<Learned>) -> Result<String, String> {
		if let Some(known) = &self.integrity {
			return known.clone();
		}
		let (hash, learnt) = integrity(&self.file).await;
		learned.extend(learnt);
		self.integrity = Some(hash.clone());
		hash
	}
}

/// Remove every copy of a duplicated content beneath the scope but the
/// first in walk order.
async fn keep_first(
	ctx: &JobContext<'_>,
	context: &CoreContext,
	duplicates: &Duplicates,
	mode: DeleteMode,
	started_at: Instant,
) -> JobResult<DeleteOutput> {
	let scope = duplicates
		.scope
		.as_ref()
		.ok_or_else(|| JobError::execution("keeping the first copy needs a folder to look in"))?;
	let index = context.volume_index();
	let reaches = stores_beneath(context, scope).await;
	if reaches.is_empty() {
		return Err(JobError::execution(format!(
			"{scope} is not in a tracked source"
		)));
	}
	let min_size = duplicates.min_size.unwrap_or(0);
	let duplicated = duplicated_in(index, &reaches, min_size)
		.await
		.map_err(failed)?;
	ctx.log(format!(
		"{} contents held more than once beneath {scope}",
		duplicated.len()
	));

	let after: Option<CompareCursor> = ctx.load_state().await?;
	let mut keepers: HashMap<String, Keeper> = HashMap::new();
	if let Some(cursor) = &after {
		ctx.log("Resuming from the last checkpoint");
		let cursor: Key = (cursor.directory.clone(), cursor.name.clone());
		let mut before = Folder::open(index, reaches.clone(), true, None)
			.await
			.map_err(failed)?;
		while let Some(file) = before.next().await.map_err(failed)? {
			if file.key > cursor {
				break;
			}
			if let Some(sampled) = file.entry.sampled_hash.clone() {
				if duplicated.contains(&sampled) {
					keepers.entry(sampled).or_insert_with(|| Keeper::new(file));
				}
			}
		}
	}

	ctx.progress(Progress::Indeterminate("Counting".to_string()));
	let (total, total_bytes) = surplus(index, &reaches, &duplicated)
		.await
		.map_err(failed)?
		.into_iter()
		.filter(|(_, keeper)| !keeper)
		.fold((0u64, 0u64), |(files, bytes), (file, _)| {
			(files + 1, bytes + size_of(&file.entry))
		});
	ctx.log(format!(
		"{total} surplus copies ({total_bytes} bytes) to {}",
		mode.label()
	));

	let mut folder = Folder::open(index, reaches, true, after.as_ref())
		.await
		.map_err(failed)?;
	let mut tally = Tally::default();
	let mut handled = 0u64;
	loop {
		ctx.check_interrupt().await?;
		let mut batch = Vec::with_capacity(BATCH);
		let mut cursor: Option<Key> = None;
		let mut drained = false;
		while batch.len() < BATCH {
			let Some(file) = folder.next().await.map_err(failed)? else {
				drained = true;
				break;
			};
			cursor = Some(file.key.clone());
			let Some(sampled) = file.entry.sampled_hash.clone() else {
				continue;
			};
			if !duplicated.contains(&sampled) {
				continue;
			}
			if keepers.contains_key(&sampled) {
				batch.push(file);
			} else {
				keepers.insert(sampled, Keeper::new(file));
			}
		}
		if !batch.is_empty() {
			handled += batch.len() as u64;
			settle_and_remove(ctx, context, &mut keepers, batch, &mode, &mut tally).await?;
			ctx.progress(progress(handled, total, total_bytes, &tally));
		}
		if let Some((directory, name)) = cursor {
			ctx.checkpoint_with_state(&CompareCursor { directory, name })
				.await?;
		}
		if drained {
			break;
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

/// Remove every other copy, in the scope, of the content of each chosen
/// file.
async fn keep_these(
	ctx: &JobContext<'_>,
	context: &CoreContext,
	duplicates: &Duplicates,
	paths: &[SdPath],
	mode: DeleteMode,
	started_at: Instant,
) -> JobResult<DeleteOutput> {
	let index = context.volume_index();
	let mut tally = Tally::default();
	let mut keepers: HashMap<String, Keeper> = HashMap::new();
	for path in paths {
		match keeper_at(&context.volume_manager, index, path).await {
			Some(file) => match file.entry.sampled_hash.clone() {
				Some(sampled) => {
					keepers.entry(sampled).or_insert_with(|| Keeper::new(file));
				}
				None => tally.skip(file.path.clone(), SkipReason::Unhashed),
			},
			None => tally.skip(
				path.as_local_path()
					.map(Path::to_path_buf)
					.unwrap_or_default(),
				SkipReason::Unhashed,
			),
		}
	}
	for skip in &tally.skipped {
		ctx.log(format!(
			"Cannot keep {}: it {}",
			skip.path.display(),
			skip.reason
		));
	}

	let reaches = match &duplicates.scope {
		Some(scope) => stores_beneath(context, scope).await,
		None => every_store(context),
	};
	let kept: HashSet<PathBuf> = keepers
		.values()
		.map(|keeper| keeper.file.path.clone())
		.collect();
	let hashes: Vec<String> = keepers.keys().cloned().collect();
	let candidates = other_copies(
		index,
		&reaches,
		&hashes,
		&kept,
		duplicates.min_size.unwrap_or(0),
	)
	.await
	.map_err(failed)?;
	let total = candidates.len() as u64;
	let total_bytes: u64 = candidates.iter().map(|copy| size_of(&copy.entry)).sum();
	ctx.log(format!(
		"{total} other copies ({total_bytes} bytes) of {} kept files to {}",
		keepers.len(),
		mode.label()
	));

	let mut handled = 0u64;
	let mut candidates = candidates.into_iter();
	loop {
		ctx.check_interrupt().await?;
		let batch: Vec<Keyed> = candidates.by_ref().take(BATCH).collect();
		if batch.is_empty() {
			break;
		}
		handled += batch.len() as u64;
		settle_and_remove(ctx, context, &mut keepers, batch, &mode, &mut tally).await?;
		ctx.progress(progress(handled, total, total_bytes, &tally));
	}

	ctx.log(format!(
		"Delete operation completed: {} deleted, {} skipped, {} failed",
		tally.deleted,
		tally.skipped.len(),
		tally.failed.len()
	));
	Ok(tally.into_output(mode, started_at))
}

/// Settle a batch of surplus copies against their keepers, remove those
/// that may go, and record what the reads learned.
async fn settle_and_remove(
	ctx: &JobContext<'_>,
	context: &CoreContext,
	keepers: &mut HashMap<String, Keeper>,
	batch: Vec<Keyed>,
	mode: &DeleteMode,
	tally: &mut Tally,
) -> JobResult<()> {
	let mut learned = Vec::new();
	let skipped_before = tally.skipped.len();
	let removable = settle(keepers, batch, tally, &mut learned).await;
	for skip in &tally.skipped[skipped_before..] {
		ctx.log(format!("Left {}: {}", skip.path.display(), skip.reason));
	}

	let paths: Vec<SdPath> = removable
		.iter()
		.map(|file| SdPath::local(file.path.clone()))
		.collect();
	if !paths.is_empty() {
		let strategy =
			DeleteStrategyRouter::select_strategy(&paths, ctx.volume_manager().as_deref()).await;
		let results = strategy
			.execute(ctx, &paths, mode.clone())
			.await
			.map_err(|e| JobError::execution(format!("Strategy execution failed: {e}")))?;
		let effects = tally.record(results);
		ctx.record(effects).await;
	}
	record_learned(ctx, context, learned).await;
	Ok(())
}

/// Which of a batch of surplus copies may go: those whose bytes, read in
/// full, match their keeper's. The rest are skipped with why.
async fn settle(
	keepers: &mut HashMap<String, Keeper>,
	batch: Vec<Keyed>,
	tally: &mut Tally,
	learned: &mut Vec<Learned>,
) -> Vec<Keyed> {
	let mut removable = Vec::new();
	let mut reads = futures::stream::iter(batch)
		.map(|file| async move {
			let (hash, learnt) = integrity(&file).await;
			(file, hash, learnt)
		})
		.buffered(READS);
	while let Some((file, hash, learnt)) = reads.next().await {
		let keeper = file
			.entry
			.sampled_hash
			.as_ref()
			.and_then(|sampled| keepers.get_mut(sampled));
		let Some(keeper) = keeper else {
			tally.skip(file.path.clone(), SkipReason::NoCopy);
			continue;
		};
		let verdict = match (keeper.integrity(learned).await, hash) {
			(Ok(theirs), Ok(ours)) if theirs == ours => Ok(()),
			(Ok(_), Ok(_)) => Err(SkipReason::Differs),
			(Err(error), _) | (_, Err(error)) => Err(SkipReason::Unreadable(error)),
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
	removable
}

/// The sampled hashes of contents held more than once beneath the reaches,
/// within one store at a time.
pub(super) async fn duplicated_in(
	index: &VolumeIndex,
	reaches: &[Reach],
	min_size: u64,
) -> Result<HashSet<String>, QueryError> {
	let mut duplicated = HashSet::new();
	for reach in reaches {
		let db = index
			.read_store(reach.source.id)
			.await
			.ok_or_else(|| unreadable(&reach.source.root))?;
		duplicated.extend(
			sd_store::read::duplicated_contents_beneath(db.pool(), &reach.scope, min_size as i64)
				.await
				.map_err(read_failed)?,
		);
	}
	Ok(duplicated)
}

/// Every file beneath the reaches holding a duplicated content, in walk
/// order, flagged where it is the first holder of its content and so the
/// copy that stays.
pub(super) async fn surplus(
	index: &VolumeIndex,
	reaches: &[Reach],
	duplicated: &HashSet<String>,
) -> Result<Vec<(Keyed, bool)>, QueryError> {
	let mut folder = Folder::open(index, reaches.to_vec(), true, None).await?;
	let mut seen: HashSet<String> = HashSet::new();
	let mut files = Vec::new();
	while let Some(file) = folder.next().await? {
		let Some(sampled) = &file.entry.sampled_hash else {
			continue;
		};
		if !duplicated.contains(sampled) {
			continue;
		}
		let keeper = seen.insert(sampled.clone());
		files.push((file, keeper));
	}
	Ok(files)
}

/// The file at `path` as its store holds it, for a copy chosen to stay.
pub(super) async fn keeper_at(
	volumes: &VolumeManager,
	index: &VolumeIndex,
	path: &SdPath,
) -> Option<Keyed> {
	for reach in stores_beneath_in(volumes, index, path).await {
		if !reach.prefix.is_empty() {
			continue;
		}
		let Some(db) = index.read_store(reach.source.id).await else {
			continue;
		};
		if let Ok(Some(entry)) = sd_store::read::entry_by_path(db.pool(), &reach.scope).await {
			return Some(Keyed::in_source(entry, Arc::new(reach.source.root.clone())));
		}
	}
	None
}

/// Every copy beneath the reaches of the contents in `hashes`, other than
/// the kept paths, of at least `min_size` bytes.
pub(super) async fn other_copies(
	index: &VolumeIndex,
	reaches: &[Reach],
	hashes: &[String],
	kept: &HashSet<PathBuf>,
	min_size: u64,
) -> Result<Vec<Keyed>, QueryError> {
	let mut copies = Vec::new();
	for reach in reaches {
		let Some(db) = index.read_store(reach.source.id).await else {
			continue;
		};
		let root = Arc::new(reach.source.root.clone());
		for entry in sd_store::read::copies_beneath(db.pool(), hashes, &reach.scope)
			.await
			.map_err(read_failed)?
		{
			if size_of(&entry) < min_size {
				continue;
			}
			let file = Keyed::in_source(entry, root.clone());
			if kept.contains(&file.path) {
				continue;
			}
			copies.push(file);
		}
	}
	Ok(copies)
}

pub(super) fn size_of(entry: &FsEntry) -> u64 {
	entry.size.unwrap_or(0).max(0) as u64
}

fn progress(handled: u64, total: u64, total_bytes: u64, tally: &Tally) -> Progress {
	Progress::generic(
		GenericProgress::new(
			(handled as f32 / total.max(1) as f32).min(1.0),
			"Deleting",
			format!("{handled} of {total} copies"),
		)
		.with_completion(handled, total)
		.with_bytes(tally.bytes, total_bytes)
		.with_errors(tally.failed.len() as u64, tally.skipped.len() as u64),
	)
}

fn unreadable(root: &Path) -> QueryError {
	QueryError::Internal(format!("no readable index for {}", root.display()))
}

fn read_failed(error: sd_store::Error) -> QueryError {
	QueryError::Internal(error.to_string())
}

fn failed(error: QueryError) -> JobError {
	JobError::execution(error.to_string())
}
