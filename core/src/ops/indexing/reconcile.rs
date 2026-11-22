//! # Warm-start reconcile
//!
//! A restart restores each source's map from its snapshot or its store, and the
//! watcher only hears about changes from that moment on. Whatever happened
//! while the daemon was stopped is invisible to both. This pass closes that
//! gap: it reads every directory of an attached source, compares size and
//! modification time with what the map holds, and applies the differences
//! through the same `ArenaWriter` the watcher uses, so the arena, the store and
//! the clients hear one description of each change.
//!
//! Unchanged files cost a `stat` and nothing else. Identity comes from the
//! store's ledger, so a file renamed while the daemon was stopped keeps its
//! record (and its tags) when its inode, size and modification time agree,
//! and only new or modified files are queued for hashing. One case resolves
//! fresh: two paths of different kinds swapped through a temporary name. Each
//! path still holds the other's old binding when the arrivals resolve, so
//! the swapped entries get new records and are hashed again.
//!
//! Removals follow the sweep rules every walk follows. Nothing is removed
//! unless the pass read every directory it needed to, a pass that would remove
//! more than half of what it compared removes nothing, and each removal is
//! checked against the disk once more before it is applied, since the watcher
//! is live while this runs. That check reads the name the directory stores,
//! because a case-insensitive volume answers a stat for the old spelling of a
//! case-only rename.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::context::CoreContext;
use crate::infra::job::{generic_progress::GenericProgress, prelude::*};
use crate::ops::indexing::metadata::{
	extract_metadata, is_provider_placeholder, map_spelling, EntryMetadata,
};
use crate::ops::indexing::rules::{
	build_default_ruler, IndexerRuler, MetadataForIndexerRules, RuleToggles, RulerDecision,
};
use crate::ops::indexing::state::EntryKind;
use crate::ops::indexing::{ArenaWriter, Notify, Seen};

/// Observations per writer batch.
const BATCH: usize = 1_000;

/// How long to wait for a map still filling from its store.
const FILL_WAIT: Duration = Duration::from_secs(600);

/// How often a long reconcile logs how far it got.
const PROGRESS_EVERY: Duration = Duration::from_secs(30);

/// How often a reconcile running as a job reports its progress.
const PROGRESS_TICK: Duration = Duration::from_millis(500);

/// How long one directory listing may take before the pass moves on without
/// it. A healthy local listing answers in milliseconds; one that waits on a
/// network or a cloud provider can wait for minutes and still fail.
pub const READ_BOUND: Duration = Duration::from_secs(10);

/// How often directories waiting on access are read again when nothing
/// announces an answer. A prompt that was refused and later allowed in
/// System Settings unblocks no listing, so only this notices it.
pub const ACCESS_RETRY: Duration = Duration::from_secs(300);

/// What a bounded directory listing came back with.
pub enum Bounded<T> {
	Answered(Result<T, tokio::task::JoinError>),
	/// The listing did not answer within the bound and is still running.
	Unanswered(tokio::task::JoinHandle<T>),
}

/// Wait up to `bound` for a listing running on its own task.
///
/// A listing behind a macOS privacy prompt nobody answers blocks in the
/// kernel until someone does, and one on a stalled network mount can block
/// for minutes. Neither can be cancelled, so the caller moves on and keeps
/// the handle to learn when the listing returns.
pub async fn bounded<T>(bound: Duration, mut listing: tokio::task::JoinHandle<T>) -> Bounded<T> {
	match tokio::time::timeout(bound, &mut listing).await {
		Ok(answered) => Bounded::Answered(answered),
		Err(_) => Bounded::Unanswered(listing),
	}
}

/// What one reconcile found and applied.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconcileReport {
	pub directories: u64,
	pub compared: u64,
	pub added: u64,
	pub modified: u64,
	pub moved: u64,
	pub removed: u64,
	/// Removals withheld because the pass could not trust its own view.
	pub removals_refused: bool,
	/// Cloud provider placeholders compared but not listed.
	#[serde(default)]
	pub placeholders: u64,
	/// Directories the map holds that the rules now keep out. They are left
	/// unread and what the map held under them is removed.
	#[serde(default)]
	pub excluded: u64,
}

/// How far a running reconcile has got.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReconcileProgress {
	pub directories: u64,
	pub compared: u64,
	/// Directories found but not read yet.
	pub pending: u64,
	/// Changes found so far.
	pub changes: u64,
	/// Whether the pass is applying what it found rather than reading.
	pub applying: bool,
}

/// One entry a fresh read of a directory found.
pub struct OnDisk {
	path: PathBuf,
	kind: Option<EntryKind>,
	size: u64,
	modified: Option<SystemTime>,
	placeholder: bool,
}

/// How a pass reads the disk and where it reports. Tests swap the reader to
/// stand in for a directory that never answers.
pub struct Pass {
	pub read: fn(&Path) -> std::io::Result<Vec<OnDisk>>,
	pub read_bound: Duration,
	pub progress: Option<watch::Sender<ReconcileProgress>>,
	/// Directories to read instead of the whole source, such as those a walk
	/// moved past. Empty reads everything from the root.
	pub within: Vec<PathBuf>,
}

impl Default for Pass {
	fn default() -> Self {
		Self {
			read: read_directory,
			read_bound: READ_BOUND,
			progress: None,
			within: Vec::new(),
		}
	}
}

/// The rules the walk applies under one directory, built on first use.
struct DirectoryRules<'a> {
	toggles: RuleToggles,
	root: &'a Path,
	dir: &'a Path,
	ruler: Option<IndexerRuler>,
}

struct IsDirectory(bool);

impl MetadataForIndexerRules for IsDirectory {
	fn is_dir(&self) -> bool {
		self.0
	}
}

impl DirectoryRules<'_> {
	/// Whether the walk would keep this entry out of the map. One ruler per
	/// directory, as the walk builds it, because building one reads the
	/// `.gitignore` files above the directory.
	async fn reject(&mut self, path: &Path, kind: EntryKind) -> bool {
		if self.ruler.is_none() {
			self.ruler = Some(build_default_ruler(self.toggles, self.root, self.dir).await);
		}
		let Some(ruler) = &self.ruler else {
			return false;
		};
		matches!(
			ruler
				.evaluate_path(path, &IsDirectory(kind == EntryKind::Directory))
				.await,
			Ok(RulerDecision::Reject)
		)
	}
}

fn seconds(time: Option<SystemTime>) -> Option<u64> {
	time.and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
		.map(|since| since.as_secs())
}

/// Read one directory. An entry whose metadata cannot be read is still
/// reported, with no kind, so it is never mistaken for a deletion.
pub fn read_directory(dir: &Path) -> std::io::Result<Vec<OnDisk>> {
	let mut entries = Vec::new();
	for entry in std::fs::read_dir(dir)? {
		let Ok(entry) = entry else {
			continue;
		};
		let path = entry.path();
		match std::fs::symlink_metadata(&path) {
			Ok(metadata) => entries.push(OnDisk {
				kind: Some(if metadata.is_dir() {
					EntryKind::Directory
				} else if metadata.is_symlink() {
					EntryKind::Symlink
				} else {
					EntryKind::File
				}),
				size: metadata.len(),
				modified: metadata.modified().ok(),
				placeholder: is_provider_placeholder(&metadata),
				path,
			}),
			Err(_) => entries.push(OnDisk {
				path,
				kind: None,
				size: 0,
				modified: None,
				placeholder: false,
			}),
		}
	}
	Ok(entries)
}

/// How an old path's identities fared against what arrived.
struct Weighed {
	/// Entries at and under the path.
	held: u64,
	/// The path's own identity arrived somewhere else.
	own_arrived: bool,
	/// Entries under the path whose identity did not arrive.
	beneath: u64,
}

fn weigh(index: &crate::ops::indexing::Arena, path: &Path, arrived: &HashSet<Uuid>) -> Weighed {
	let identities = index.subtree_identities(path);
	let came = |uuid: &Option<Uuid>| uuid.is_some_and(|uuid| arrived.contains(&uuid));
	Weighed {
		held: identities.len().max(1) as u64,
		own_arrived: identities.first().is_some_and(came),
		beneath: identities.iter().skip(1).filter(|uuid| !came(uuid)).count() as u64,
	}
}

/// Hand paths to the writer in batches and note the identities they resolve
/// to. Returns how many were still there to observe.
async fn arrive(writer: &ArenaWriter, paths: &[PathBuf], arrived: &mut HashSet<Uuid>) -> u64 {
	let mut count = 0;
	for chunk in paths.chunks(BATCH) {
		let mut batch: Vec<EntryMetadata> = Vec::with_capacity(chunk.len());
		for path in chunk {
			// Gone again since the read; the watcher answers for it.
			if let Ok(metadata) = extract_metadata(path, None).await {
				batch.push(metadata);
			}
		}
		count += batch.len() as u64;
		for change in writer.apply(Seen::Entries(batch)).await {
			arrived.insert(change.uuid);
		}
	}
	count
}

/// Reconcile every attached source with the disk, one after another.
///
/// Runs at startup once the maps are restored. A source with no map coverage
/// is left for the coverage heal, and one already being walked is left to its
/// walk.
pub async fn reconcile_attached_sources(context: Arc<CoreContext>) {
	let cache = context.volume_index();
	let sources = cache.sources();
	info!(
		sources = sources.len(),
		"reconciling registered sources with the disk"
	);
	for source in sources {
		if !source.attached {
			info!(
				source = %source.id,
				root = %source.root.display(),
				volume = ?source.volume_uuid,
				volume_state = ?source.volume_state,
				"not reconciling: the source is not attached"
			);
			continue;
		}
		run_reconcile(
			&context,
			source.library,
			&source.root,
			source.entry_count == Some(0),
		)
		.await;
	}
}

/// Read again every directory a walk or a reconcile moved past because its
/// listing did not answer, once nothing is still blocked on it.
///
/// Each directory is read by a reconcile limited to it, under the innermost
/// source holding it so what arrives goes through the same writer, store and
/// hashing as any other change. A directory no source holds, such as one the
/// drive map or a browse moved past, is reconciled under its drive's map.
///
/// The bound is twice what the last listing took, and never less than
/// [`READ_BOUND`]: a listing that is slow rather than blocked, such as one
/// on a slow network mount, answers the next read instead of timing out
/// on every retry.
pub async fn revisit_awaiting_access(context: &Arc<CoreContext>) {
	let cache = context.volume_index();
	let dirs = cache.awaiting_access_unblocked();
	if dirs.is_empty() {
		return;
	}
	let sources: Vec<_> = cache
		.sources()
		.into_iter()
		.filter(|source| source.attached)
		.collect();
	struct Revisit {
		library: Option<Uuid>,
		within: Vec<PathBuf>,
		bound: Duration,
	}
	let mut by_root: std::collections::HashMap<PathBuf, Revisit> = std::collections::HashMap::new();
	for (dir, took) in dirs {
		let (root, library) = match crate::ops::paths::reach::innermost_source(&sources, &dir) {
			Some(source) => (source.root.clone(), source.library),
			None => (
				cache.resolve(&dir).root().unwrap_or_else(|| dir.clone()),
				None,
			),
		};
		let revisit = by_root.entry(root).or_insert_with(|| Revisit {
			library,
			within: Vec::new(),
			bound: READ_BOUND,
		});
		revisit.within.push(dir);
		revisit.bound = revisit.bound.max(took * 2);
	}
	for (
		root,
		Revisit {
			library,
			within,
			bound,
		},
	) in by_root
	{
		// A walk under way reads these itself.
		if cache.is_indexing(&root) {
			continue;
		}
		info!(
			root = %root.display(),
			directories = within.len(),
			"reading again what was waiting on access"
		);
		reconcile_source_with(
			context,
			library,
			&root,
			false,
			Pass {
				within,
				read_bound: bound,
				..Pass::default()
			},
		)
		.await;
	}
}

/// Reconcile one source as a job of its library, so job lists, the app and
/// the CLI show the pass and its progress, and wait for it to finish. A
/// source no open library holds is reconciled directly.
pub async fn run_reconcile(
	context: &Arc<CoreContext>,
	library: Option<Uuid>,
	root: &Path,
	walked_empty: bool,
) {
	let library_handle = match library {
		Some(id) => context.libraries().await.get_library(id).await,
		None => None,
	};
	let Some(library_handle) = library_handle else {
		reconcile_source(context, library, root, walked_empty).await;
		return;
	};
	let job = ReconcileJob {
		root: root.to_path_buf(),
		walked_empty,
	};
	match library_handle.jobs().dispatch(job).await {
		Ok(handle) => {
			if let Err(error) = handle.wait().await {
				warn!(root = %root.display(), %error, "the reconcile job did not finish");
			}
		}
		Err(error) => {
			warn!(root = %root.display(), %error, "could not dispatch the reconcile job; reconciling inline");
			reconcile_source(context, library, root, walked_empty).await;
		}
	}
}

/// The startup reconcile of one source, as a job.
///
/// Not resumable and not persisted: every start reconciles every attached
/// source again, so a pass cut short by a stop has nothing to resume.
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct ReconcileJob {
	root: PathBuf,
	walked_empty: bool,
}

impl From<ReconcileReport> for JobOutput {
	fn from(report: ReconcileReport) -> Self {
		JobOutput::custom(report)
	}
}

impl Job for ReconcileJob {
	const NAME: &'static str = "reconcile";
	const RESUMABLE: bool = false;
	const DESCRIPTION: Option<&'static str> = Some("Bring the index up to date with the disk");
}

impl crate::infra::job::traits::DynJob for ReconcileJob {
	fn job_name(&self) -> &'static str {
		Self::NAME
	}

	fn dedup_key(&self) -> Option<String> {
		Some(format!("reconcile:{}", self.root.display()))
	}

	/// Every start runs one per source, so a row each time would bury the
	/// job history. The pass shows in the job list while it runs.
	fn should_persist(&self) -> bool {
		false
	}

	fn should_emit_events(&self) -> bool {
		true
	}
}

#[async_trait::async_trait]
impl JobHandler for ReconcileJob {
	type Output = ReconcileReport;

	async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
		let context = ctx.library().core_context().clone();
		let label = self
			.root
			.file_name()
			.map(|name| name.to_string_lossy().into_owned())
			.unwrap_or_else(|| self.root.display().to_string());
		// The map's count from the last walk or snapshot is what a pass
		// compares against, so it stands in for the total.
		let expected = context
			.volume_index()
			.sources()
			.into_iter()
			.find(|source| source.root == self.root)
			.and_then(|source| source.entry_count)
			.unwrap_or(0);

		let (sender, mut progress) = watch::channel(ReconcileProgress::default());
		let pass = reconcile_source_with(
			&context,
			Some(ctx.library().id()),
			&self.root,
			self.walked_empty,
			Pass {
				progress: Some(sender),
				..Pass::default()
			},
		);
		tokio::pin!(pass);
		let started = Instant::now();
		ctx.progress(Progress::generic(GenericProgress::new(
			0.0,
			"Reconciling",
			format!("{label}: checking for changes made while Spacedrive was closed"),
		)));
		let report = loop {
			tokio::select! {
				report = &mut pass => break report,
				changed = progress.changed() => {
					if changed.is_err() {
						break (&mut pass).await;
					}
					let now = *progress.borrow_and_update();
					// Interrupting while the pass applies what it found would
					// leave half a batch written; reading has written nothing.
					if !now.applying {
						ctx.check_interrupt().await?;
					}
					ctx.progress(Progress::generic(reconcile_progress(&label, now, expected, started.elapsed())));
				}
			}
		};

		match report {
			Some(report) => Ok(report),
			None => {
				ctx.log("Skipped; daemon.log names the reason");
				Ok(ReconcileReport::default())
			}
		}
	}
}

fn reconcile_progress(
	label: &str,
	now: ReconcileProgress,
	expected: u64,
	elapsed: Duration,
) -> GenericProgress {
	if now.applying {
		return GenericProgress::new(
			0.99,
			"Applying",
			format!("{label}: applying {} changes", now.changes),
		);
	}
	let total = expected.max(now.compared);
	let rate = now.compared as f32 / elapsed.as_secs_f32().max(0.001);
	let remaining = (rate > 0.0)
		.then(|| Duration::from_secs_f32(total.saturating_sub(now.compared) as f32 / rate));
	GenericProgress::new(
		0.0,
		"Reconciling",
		format!(
			"{label}: {} of about {total} entries checked, {} changes found",
			now.compared, now.changes
		),
	)
	.with_completion(now.compared, total)
	.with_performance(rate, remaining, Some(elapsed))
}

/// Bring one source's map and store in line with its disk.
///
/// `walked_empty` says a completed walk found the source empty, so a map
/// with nothing under the root is coverage rather than a gap.
pub async fn reconcile_source(
	context: &Arc<CoreContext>,
	library: Option<Uuid>,
	root: &Path,
	walked_empty: bool,
) -> Option<ReconcileReport> {
	reconcile_source_with(context, library, root, walked_empty, Pass::default()).await
}

/// [`reconcile_source`] with its reader, read bound and progress channel.
pub async fn reconcile_source_with(
	context: &Arc<CoreContext>,
	library: Option<Uuid>,
	root: &Path,
	walked_empty: bool,
	pass: Pass,
) -> Option<ReconcileReport> {
	let cache = context.volume_index();
	cache.ensure_restored(root).await;

	// A store rebuild fills the map in the background, and a restore can
	// start a browse of a root it holds nothing under; comparing against a
	// half-built map would read the rest as new.
	let busy = || cache.is_filling_from_store(root) || cache.is_indexing(root);
	let waited = Instant::now();
	if busy() {
		info!(
			root = %root.display(),
			filling_from_store = cache.is_filling_from_store(root),
			walking = cache.is_indexing(root),
			"reconcile waiting for the map to finish building"
		);
	}
	while busy() && waited.elapsed() < FILL_WAIT {
		tokio::time::sleep(Duration::from_millis(200)).await;
	}
	if busy() {
		info!(
			root = %root.display(),
			filling_from_store = cache.is_filling_from_store(root),
			walking = cache.is_indexing(root),
			waited_secs = waited.elapsed().as_secs(),
			"not reconciling: the map is still being built"
		);
		return None;
	}

	let partition = cache.resolve(root);
	let index = partition.index();
	let covered = index
		.read()
		.await
		.known_children(root)
		.is_some_and(|children| !children.is_empty());
	if !covered && !walked_empty && pass.within.is_empty() {
		info!(
			root = %root.display(),
			map_root = ?partition.root(),
			"not reconciling: the map holds nothing under the root, left to the heal"
		);
		return None;
	}

	let started = Instant::now();
	let rule_toggles = cache.rule_toggles_for(root);
	let mut report = ReconcileReport::default();
	let mut changed: Vec<PathBuf> = Vec::new();
	let mut added: Vec<PathBuf> = Vec::new();
	let mut missing: Vec<(PathBuf, bool, u64)> = Vec::new();
	let mut replaced: Vec<(PathBuf, bool, u64)> = Vec::new();
	let mut excluded: Vec<PathBuf> = Vec::new();
	let mut unreadable = false;
	let mut known_entries: u64 = 0;

	info!(root = %root.display(), "reconciling source with the disk");
	let mut last_progress = Instant::now();
	let mut last_tick = Instant::now();
	let mut pending = if pass.within.is_empty() {
		VecDeque::from([root.to_path_buf()])
	} else {
		VecDeque::from(pass.within.clone())
	};
	while let Some(dir) = pending.pop_front() {
		if let Some(progress) = &pass.progress {
			if last_tick.elapsed() >= PROGRESS_TICK {
				last_tick = Instant::now();
				progress.send_replace(ReconcileProgress {
					directories: report.directories,
					compared: report.compared,
					pending: pending.len() as u64,
					changes: (added.len() + changed.len()) as u64,
					applying: false,
				});
			}
		}
		// A large source takes minutes to read, and nothing else shows the pass is alive.
		if last_progress.elapsed() >= PROGRESS_EVERY {
			last_progress = Instant::now();
			info!(
				root = %root.display(),
				directories = report.directories,
				compared = report.compared,
				pending = pending.len(),
				added = added.len(),
				changed = changed.len(),
				"reconcile still reading the source"
			);
		}
		// A listing that waits on a provider, a network or a privacy prompt
		// can block for minutes; the pass moves on and the listing finishes
		// on its own.
		// A listing of this directory is still blocked from an earlier read;
		// another would only add a second blocked thread.
		if cache.access_blocked_needed(&dir) {
			unreadable = true;
			continue;
		}
		let listing = {
			let dir = dir.clone();
			let read = pass.read;
			tokio::task::spawn_blocking(move || read(&dir))
		};
		let on_disk = match bounded(pass.read_bound, listing).await {
			Bounded::Answered(Ok(Ok(entries))) => {
				cache.access_settled(&dir);
				entries
			}
			// Gone since its parent was read. Nothing under it was compared,
			// so nothing under it is counted missing; the watcher has it.
			Bounded::Answered(Ok(Err(error))) if error.kind() == std::io::ErrorKind::NotFound => {
				debug!(dir = %dir.display(), "a directory went away during the reconcile");
				cache.access_settled(&dir);
				continue;
			}
			// A directory already waiting on access keeps waiting: a refused
			// prompt answers with an error until it is allowed in settings.
			Bounded::Answered(Ok(Err(error))) => {
				warn!(dir = %dir.display(), %error, "reconcile could not read a directory");
				unreadable = true;
				continue;
			}
			Bounded::Answered(Err(error)) => {
				warn!(dir = %dir.display(), %error, "reconcile read task failed");
				unreadable = true;
				continue;
			}
			Bounded::Unanswered(listing) => {
				warn!(
					dir = %dir.display(),
					bound_secs = pass.read_bound.as_secs_f32(),
					"reconcile moved past a directory that did not answer in time"
				);
				cache.await_access(dir.clone(), pass.read_bound, listing);
				unreadable = true;
				continue;
			}
		};
		report.directories += 1;
		let mut rules = DirectoryRules {
			toggles: rule_toggles,
			root,
			dir: &dir,
			ruler: None,
		};

		let known = index.read().await.known_children(&map_spelling(&dir));
		let mut known: std::collections::HashMap<PathBuf, crate::ops::indexing::arena::KnownChild> =
			known
				.unwrap_or_default()
				.into_iter()
				.map(|child| (child.path.clone(), child))
				.collect();
		known_entries += known.len() as u64;

		for entry in on_disk {
			let key = map_spelling(&entry.path).into_owned();
			if crate::config::is_managed(&entry.path) {
				known.remove(&key);
				continue;
			}
			let Some(kind) = entry.kind else {
				known.remove(&key);
				continue;
			};
			match known.remove(&key) {
				Some(held) if held.kind == kind => {
					report.compared += 1;
					if kind == EntryKind::Directory {
						if held.summarised {
							continue;
						}
						if entry.placeholder {
							report.placeholders += 1;
						} else if rules.reject(&entry.path, kind).await {
							// Walked before a rule that now keeps it out. A
							// source walk applies rules and so never sweeps,
							// which leaves this pass to clear what is under it.
							report.excluded += 1;
							excluded.push(entry.path);
						} else {
							pending.push_back(entry.path);
						}
					} else if held.size != entry.size
						|| seconds(held.modified) != seconds(entry.modified)
					{
						changed.push(entry.path);
					}
				}
				held => {
					if let Some(held) = held {
						// A file replaced by a directory, or the reverse. The
						// path is there, so this is no absence for the fuse to
						// weigh, but the old entry and anything under it go.
						replaced.push((held.path, held.kind == EntryKind::Directory, held.entries));
					}
					if rules.reject(&entry.path, kind).await {
						continue;
					}
					if kind == EntryKind::Directory {
						if entry.placeholder {
							report.placeholders += 1;
						} else {
							pending.push_back(entry.path.clone());
						}
					}
					added.push(entry.path);
				}
			}
		}

		for (path, held) in known {
			missing.push((path, held.kind == EntryKind::Directory, held.entries));
		}
	}

	if let Some(progress) = &pass.progress {
		progress.send_replace(ReconcileProgress {
			directories: report.directories,
			compared: report.compared,
			pending: 0,
			changes: (added.len() + changed.len() + missing.len() + replaced.len()) as u64,
			applying: true,
		});
	}

	let store = cache.store_for(root).await;
	let writer = ArenaWriter::new(index.clone(), context.events.clone(), store.clone())
		.notifying(Notify::Batched);
	let quiet = ArenaWriter::new(index.clone(), context.events.clone(), store.clone())
		.notifying(Notify::Silent);
	let loud = ArenaWriter::new(index.clone(), context.events.clone(), store.clone());

	// Parents come before their children in breadth-first order, so a new
	// directory's record exists before the files under it resolve. Arrivals
	// at or under a path whose kind changed wait until the old entry is
	// cleared, or a directory's old children would stay under a path that is
	// now a file. The rest arrive first, so a file that moved out of a
	// replaced directory is rebound before that directory is forgotten.
	let (after_replacement, added): (Vec<PathBuf>, Vec<PathBuf>) =
		added.into_iter().partition(|path| {
			let key = map_spelling(path);
			replaced.iter().any(|(old, ..)| key.starts_with(old))
		});
	let mut arrived: HashSet<Uuid> = HashSet::new();
	report.added += arrive(&writer, &added, &mut arrived).await;
	report.modified += arrive(&writer, &changed, &mut arrived).await;

	// What leaves with an old path is every identity at or under it that did
	// not arrive somewhere else. For a replaced path the path itself is
	// affirmatively gone, so only what was under it counts. A missing
	// directory was never descended into, so its subtree counts toward what
	// the map held only here. Weighed now, before the arrivals that wait on a
	// replacement, the count can only overstate what leaves.
	let (doomed, replacing) = {
		let index = index.read().await;
		let mut doomed = 0u64;
		for (path, ..) in &missing {
			let weighed = weigh(&index, path, &arrived);
			known_entries += weighed.held - 1;
			doomed += weighed.beneath + u64::from(!weighed.own_arrived);
		}
		let mut replacing = Vec::new();
		for (path, is_directory, _) in replaced {
			let weighed = weigh(&index, &path, &arrived);
			known_entries += weighed.held - 1;
			doomed += weighed.beneath;
			replacing.push((path, is_directory, weighed));
		}
		(doomed, replacing)
	};

	let destructive = doomed > 0;
	let walk_owns_removals = cache.is_indexing(root);
	let refused = if walk_owns_removals {
		debug!(root = %root.display(), "a walk started under the reconcile; its sweep owns removals");
		true
	} else if unreadable && destructive {
		warn!(
			root = %root.display(),
			withheld = missing.len() + replacing.len(),
			"reconcile left removals alone: part of the source could not be read"
		);
		true
	} else if known_entries > 100 && doomed * 2 > known_entries {
		error!(
			root = %root.display(),
			doomed,
			known = known_entries,
			"reconcile refused its removals: it would remove more than half of the source"
		);
		true
	} else {
		false
	};

	// The old entry at a replaced path is cleared before its replacement
	// arrives, or a directory's old children would stay under a path that is
	// now a file. Missing paths are forgotten only after every arrival,
	// including those under a replacement, so a file that moved into one
	// keeps its record.
	if !refused {
		for (path, is_directory, weighed) in &replacing {
			let writer = if weighed.own_arrived { &quiet } else { &loud };
			writer
				.apply(Seen::Lost {
					path: path.clone(),
					is_directory: *is_directory,
				})
				.await;
			report.removed += 1;
		}
		report.added += arrive(&writer, &after_replacement, &mut arrived).await;
	}

	// What is held under a directory the rules now exclude leaves, and the
	// directory stays as the walk keeps one. The rules decide this, not an
	// absence, so the fuse does not weigh it: a home folder walked before
	// `Users/*/Library` can hold more under Library than everywhere else.
	if !walk_owns_removals {
		for dir in &excluded {
			let children = index
				.read()
				.await
				.known_children(&map_spelling(dir))
				.unwrap_or_default();
			// One deletion event per direct child, since a batched writer
			// announces no deletions, so an open listing drops what left.
			// A child directory's subtree goes with it in one event.
			for child in children {
				loud.apply(Seen::Lost {
					path: child.path,
					is_directory: child.kind == EntryKind::Directory,
				})
				.await;
				report.removed += child.entries;
			}
		}
	}

	// A stat alone would keep the old spelling of a case-only rename on a
	// case-insensitive volume, so a path that still stats is checked against
	// the names its directory stores. A listing is reused while the
	// directory's modification time is unchanged and was already older than
	// the listing by more than the coarsest timestamp granularity (FAT's two
	// seconds), so a rename the watcher applies afterwards always moves it.
	// Recasing a folder of N files changed while the daemon was stopped then
	// costs N stats of the path and its parent and one listing. An error
	// reads as present, so nothing is removed on a guess.
	const GRANULARITY: Duration = Duration::from_secs(2);
	struct Listing {
		changed: SystemTime,
		listed: SystemTime,
		names: HashSet<std::ffi::OsString>,
	}
	let mut listings: std::collections::HashMap<PathBuf, Listing> =
		std::collections::HashMap::new();
	let mut still_gone = async |path: PathBuf| -> bool {
		match tokio::fs::symlink_metadata(&path).await {
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
			Err(_) => return false,
			Ok(_) => {}
		}
		let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
			return false;
		};
		let Ok(changed) = tokio::fs::metadata(parent)
			.await
			.and_then(|metadata| metadata.modified())
		else {
			return false;
		};
		let reusable = listings.get(parent).is_some_and(|listing| {
			listing.changed == changed && changed + GRANULARITY <= listing.listed
		});
		if !reusable {
			let listed = SystemTime::now();
			let dir = parent.to_path_buf();
			let names = tokio::task::spawn_blocking(move || {
				std::fs::read_dir(&dir).and_then(|entries| {
					entries
						.map(|entry| entry.map(|entry| entry.file_name()))
						.collect::<std::io::Result<HashSet<_>>>()
				})
			})
			.await;
			let Ok(Ok(names)) = names else {
				listings.remove(parent);
				return false;
			};
			listings.insert(
				parent.to_path_buf(),
				Listing {
					changed,
					listed,
					names,
				},
			);
		}
		listings
			.get(parent)
			.is_some_and(|listing| !listing.names.contains(name))
	};

	// A path all of whose identities arrived elsewhere moved, and its record
	// lives on at the new path, so the old entry is dropped without
	// announcing it as deleted, whatever the safeguards say, unless a walk
	// now owns every removal. Anything else is a removal the safeguards
	// decided on above.
	if !walk_owns_removals {
		let weighed: Vec<_> = {
			let index = index.read().await;
			missing
				.into_iter()
				.map(|(path, is_directory, _)| {
					let weighed = weigh(&index, &path, &arrived);
					(path, is_directory, weighed)
				})
				.collect()
		};
		for (path, is_directory, weighed) in weighed {
			let clean_move = weighed.own_arrived && weighed.beneath == 0;
			if (refused && !clean_move) || !still_gone(path.clone()).await {
				continue;
			}
			let writer = if weighed.own_arrived { &quiet } else { &loud };
			writer.apply(Seen::Lost { path, is_directory }).await;
			if weighed.own_arrived {
				report.moved += 1;
			} else {
				report.removed += 1;
			}
		}
	}
	if refused {
		report.removals_refused = destructive || !replacing.is_empty();
	}

	if let Some(store) = &store {
		if let Err(error) = store.flush().await {
			warn!(root = %root.display(), %error, "reconcile writes did not all land");
		}
	}
	// Only a walk's snapshot save writes the row's counts otherwise, and a
	// walk can be minutes or hours away while the store already holds what
	// this pass and the watcher added.
	if let Err(error) = cache.refresh_source_counts(root).await {
		warn!(root = %root.display(), %error, "could not record the source's counts");
	}
	// A removal announces only the entry itself, so a view open anywhere
	// beneath a removed folder, such as one the rules now exclude, would keep
	// its rows. One refresh covers them all.
	if report.removed > 0 {
		context.events.emit(crate::infra::event::Event::Refresh);
	}

	info!(
		root = %root.display(),
		directories = report.directories,
		compared = report.compared,
		added = report.added,
		modified = report.modified,
		moved = report.moved,
		removed = report.removed,
		removals_refused = report.removals_refused,
		placeholders = report.placeholders,
		excluded = report.excluded,
		took_ms = started.elapsed().as_millis() as u64,
		"reconciled source with the disk"
	);

	if report.added + report.modified > 0 {
		queue_hashing(context, library, root).await;
	}
	Some(report)
}

/// Hash what the reconcile added or changed, as the watcher's nudge does.
async fn queue_hashing(context: &Arc<CoreContext>, library: Option<Uuid>, root: &Path) {
	let Some(library) = library else {
		return;
	};
	let Some(library) = context.libraries().await.get_library(library).await else {
		return;
	};
	let Some(store) = context.volume_index().store_for(root).await else {
		return;
	};
	if store.files_needing_content_count().await.unwrap_or(1) == 0 {
		crate::ops::indexing::content_text::index_text_soon(&library, root, true).await;
		return;
	}
	if let Err(error) = library
		.jobs()
		.dispatch_with_priority(
			crate::ops::indexing::content_identity::ContentIdentityJob::background(
				root.to_path_buf(),
			),
			crate::infra::job::types::JobPriority::LOW,
			None,
		)
		.await
	{
		warn!(root = %root.display(), %error, "could not queue hashing after the reconcile");
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::action::LibraryAction;
	use crate::ops::sources::track::action::{TrackSourceAction, TrackSourceInput};

	/// Stands in for a cloud provider's folder that never answers a listing.
	fn stalls_on_cloud(dir: &Path) -> std::io::Result<Vec<OnDisk>> {
		if dir.ends_with("cloud") {
			std::thread::sleep(Duration::from_secs(5));
		}
		read_directory(dir)
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn a_directory_that_never_answers_does_not_hold_up_the_pass() {
		let data = tempfile::tempdir().expect("data dir");
		let files = tempfile::tempdir().expect("files dir");
		let root = files.path().canonicalize().expect("root");
		std::fs::create_dir(root.join(".git")).expect("git dir");
		std::fs::write(root.join(".gitignore"), "*.log\n").expect("gitignore");
		for (dir, file) in [("cloud", "remote.txt"), ("local", "here.txt")] {
			std::fs::create_dir(root.join(dir)).expect("mkdir");
			std::fs::write(root.join(dir).join(file), file).expect("write");
		}

		let core = crate::Core::new(data.path().to_path_buf())
			.await
			.expect("core");
		let library = core
			.libraries
			.create_library("Reconcile", None, core.context.clone())
			.await
			.expect("library");
		let output = TrackSourceAction::from_input(TrackSourceInput {
			path: root.clone(),
			name: None,
			overrides: Default::default(),
		})
		.expect("input")
		.execute(library.clone(), core.context.clone())
		.await
		.expect("track");
		let walk = library
			.jobs()
			.get_job(JobId(output.job_id.expect("tracking dispatched a walk")))
			.await
			.expect("walk job");
		tokio::time::timeout(Duration::from_secs(120), walk.wait())
			.await
			.expect("the walk finished")
			.expect("walk");

		std::fs::write(root.join("local/new.txt"), "new").expect("write");
		std::fs::write(root.join("local/debug.log"), "ignored").expect("write");
		std::fs::remove_file(root.join("cloud/remote.txt")).expect("remove");

		let started = Instant::now();
		let report = tokio::time::timeout(
			Duration::from_secs(120),
			reconcile_source_with(
				&core.context,
				Some(library.id()),
				&output.root,
				false,
				Pass {
					read: stalls_on_cloud,
					read_bound: Duration::from_millis(200),
					progress: None,
					within: Vec::new(),
				},
			),
		)
		.await
		.expect("the pass finished")
		.expect("the pass ran");
		let took = started.elapsed();

		assert!(
			took < Duration::from_secs(3),
			"the pass waited {took:?} on a directory that never answered"
		);
		// The watcher is live too, so the map is what both left behind.
		let index = core.context.volume_index().resolve_index(&output.root);
		let index = index.read().await;
		assert!(
			index
				.get_entry_uuid(&output.root.join("local/new.txt"))
				.is_some(),
			"the readable folder's new file arrives"
		);
		assert!(
			index
				.get_entry_uuid(&output.root.join("local/debug.log"))
				.is_none(),
			"the file .gitignore names does not"
		);
		drop(index);
		assert_eq!(
			report.removed, 0,
			"nothing under the unread folder is taken for deleted"
		);
		drop(library);
		tokio::time::timeout(Duration::from_secs(120), core.shutdown())
			.await
			.expect("shutdown finished")
			.expect("shutdown");
	}

	/// A folder whose listing never answers, as one behind a macOS privacy
	/// prompt nobody answers: the first walk finishes without it, commits
	/// everything else, reports the folder as waiting on permission, and
	/// reads it as soon as its listing answers.
	#[tokio::test(flavor = "multi_thread")]
	async fn a_walk_moves_past_a_folder_whose_listing_never_answers() {
		use crate::ops::indexing::phases::discovery::held;

		let data = tempfile::tempdir().expect("data dir");
		let files = tempfile::tempdir().expect("files dir");
		let root = files.path().canonicalize().expect("root");
		for (dir, file) in [("Desktop", "note.txt"), ("Music", "song.txt")] {
			std::fs::create_dir(root.join(dir)).expect("mkdir");
			std::fs::write(root.join(dir).join(file), file).expect("write");
		}
		std::fs::write(root.join("top.txt"), "top").expect("write");

		let core = crate::Core::new(data.path().to_path_buf())
			.await
			.expect("core");
		// The walk reads the volume's spelling of the root, which on Windows
		// drops the verbatim prefix canonicalize adds, so the hold has to name
		// the folder the same way.
		let root = core
			.context
			.volume_manager
			.locate_path(&root)
			.await
			.map_or(root, |(_, spelled)| spelled);
		let desktop = root.join("Desktop");
		held::hold(&desktop);
		let library = core
			.libraries
			.create_library("Prompt", None, core.context.clone())
			.await
			.expect("library");
		let output = TrackSourceAction::from_input(TrackSourceInput {
			path: root.clone(),
			name: None,
			overrides: Default::default(),
		})
		.expect("input")
		.execute(library.clone(), core.context.clone())
		.await
		.expect("track");
		let walk = library
			.jobs()
			.get_job(JobId(output.job_id.expect("tracking dispatched a walk")))
			.await
			.expect("walk job");
		let started = Instant::now();
		tokio::time::timeout(READ_BOUND * 6, walk.wait())
			.await
			.expect("the walk waited on a folder that never answered")
			.expect("walk");
		assert!(
			started.elapsed() < READ_BOUND * 3,
			"the walk took {:?}",
			started.elapsed()
		);

		let cache = core.context.volume_index();
		let store = cache.store_for(&root).await.expect("store");
		for path in ["top.txt", "Music/song.txt", "Desktop"] {
			assert!(
				store.contains_path(&root.join(path)).await,
				"{path} is committed though another folder never answered"
			);
		}
		assert!(
			!store.contains_path(&desktop.join("note.txt")).await,
			"nothing was read under the folder that never answered"
		);
		assert_eq!(
			cache.awaiting_access_under(&root),
			vec![desktop.clone()],
			"the folder is reported as waiting on permission"
		);

		// A reconcile while the walk's listing is still blocked starts no
		// second read of the folder: it stays unread and waiting.
		tokio::time::timeout(
			READ_BOUND * 3,
			reconcile_source(&core.context, Some(library.id()), &root, false),
		)
		.await
		.expect("the reconcile finished")
		.expect("the reconcile ran");
		assert!(
			!store.contains_path(&desktop.join("note.txt")).await,
			"the reconcile did not read the folder a blocked listing holds"
		);
		assert_eq!(cache.awaiting_access_under(&root), vec![desktop.clone()]);

		// Answering the prompt is the blocked listing returning, which wakes
		// the revisit without waiting for its timer.
		held::release(&desktop);
		tokio::time::timeout(READ_BOUND * 3, async {
			while !cache.awaiting_access_under(&root).is_empty()
				|| !store.contains_path(&desktop.join("note.txt")).await
			{
				tokio::time::sleep(Duration::from_millis(100)).await;
			}
		})
		.await
		.expect("the folder is read once its listing answers");

		drop(store);
		drop(library);
		tokio::time::timeout(Duration::from_secs(60), core.shutdown())
			.await
			.expect("shutdown finished")
			.expect("shutdown");
	}

	static PROMPT_ANSWERED: std::sync::atomic::AtomicBool =
		std::sync::atomic::AtomicBool::new(false);

	/// Stands in for a folder behind a privacy prompt nobody answers, until
	/// the test answers it. Bounded so a failed test leaves no thread behind.
	fn waits_on_a_prompt(dir: &Path) -> std::io::Result<Vec<OnDisk>> {
		let asked = Instant::now();
		while dir.ends_with("Prompted")
			&& !PROMPT_ANSWERED.load(std::sync::atomic::Ordering::Acquire)
			&& asked.elapsed() < Duration::from_secs(60)
		{
			std::thread::sleep(Duration::from_millis(20));
		}
		read_directory(dir)
	}

	/// Answers the prompt and lets the count's read go when the test ends,
	/// passing or not, so the runtime is not left waiting on their threads.
	struct Answered(PathBuf);

	impl Drop for Answered {
		fn drop(&mut self) {
			PROMPT_ANSWERED.store(true, std::sync::atomic::Ordering::Release);
			crate::ops::indexing::phases::discovery::held::release(&self.0);
		}
	}

	/// The startup case: a reconcile is left holding a folder's blocked read
	/// and a walk of the same source starts afterwards. The walk records the
	/// folder as waiting without reading it, a count of a held-back folder
	/// gives up on a directory that does not answer, and the walk finishes
	/// and saves its snapshot. The source's record count follows the store
	/// without waiting for that walk.
	#[tokio::test(flavor = "multi_thread")]
	async fn a_walk_after_a_reconcile_holds_a_folder_still_finishes() {
		use crate::ops::indexing::phases::discovery::held;

		let data = tempfile::tempdir().expect("data dir");
		let files = tempfile::tempdir().expect("files dir");
		let root = files.path().canonicalize().expect("root");
		for (dir, file) in [
			("Prompted", "secret.txt"),
			("Music", "song.txt"),
			("node_modules/pkg", "index.js"),
		] {
			std::fs::create_dir_all(root.join(dir)).expect("mkdir");
			std::fs::write(root.join(dir).join(file), file).expect("write");
		}

		let core = crate::Core::new(data.path().to_path_buf())
			.await
			.expect("core");
		let root = core
			.context
			.volume_manager
			.locate_path(&root)
			.await
			.map_or(root, |(_, spelled)| spelled);
		let prompted = root.join("Prompted");
		let counted = root.join("node_modules").join("pkg");
		let library = core
			.libraries
			.create_library("Startup", None, core.context.clone())
			.await
			.expect("library");
		let output = TrackSourceAction::from_input(TrackSourceInput {
			path: root.clone(),
			name: None,
			overrides: Default::default(),
		})
		.expect("input")
		.execute(library.clone(), core.context.clone())
		.await
		.expect("track");
		library
			.jobs()
			.get_job(JobId(output.job_id.expect("tracking dispatched a walk")))
			.await
			.expect("walk job")
			.wait()
			.await
			.expect("walk");

		let cache = core.context.volume_index();
		let records = || {
			cache
				.sources_of(library.id())
				.into_iter()
				.find(|source| source.id == output.id)
				.and_then(|source| source.entry_count)
				.unwrap_or(0)
		};
		let walked = records();
		assert!(walked > 0, "the first walk recorded its counts");

		std::fs::write(root.join("Music/new.txt"), "new").expect("write");
		std::fs::write(root.join("Music/newer.txt"), "newer").expect("write");
		let answered = Answered(counted.clone());
		tokio::time::timeout(
			Duration::from_secs(120),
			reconcile_source_with(
				&core.context,
				Some(library.id()),
				&root,
				false,
				Pass {
					read: waits_on_a_prompt,
					read_bound: Duration::from_millis(200),
					progress: None,
					within: Vec::new(),
				},
			),
		)
		.await
		.expect("the reconcile finished")
		.expect("the reconcile ran");
		assert!(
			cache.access_blocked(&prompted),
			"the reconcile holds the read"
		);
		assert_eq!(
			records(),
			walked + 2,
			"the record count follows the store before the next walk saves"
		);

		let snapshot = cache.snapshot_path_for(&root).expect("snapshot path");
		let saved_before = std::fs::metadata(&snapshot)
			.and_then(|meta| meta.modified())
			.expect("the first walk saved a snapshot");
		held::hold(&counted);
		let started = Instant::now();
		let walk = crate::ops::sources::track::action::dispatch_source_walk(
			&library,
			&core.context,
			output.id,
			output.root.clone(),
			output.whole_volume,
			false,
		)
		.await
		.expect("rewalk");
		tokio::time::timeout(
			READ_BOUND * 6,
			library
				.jobs()
				.get_job(JobId(walk))
				.await
				.expect("rewalk job")
				.wait(),
		)
		.await
		.expect("the walk waited on a read another pass holds")
		.expect("walk");
		assert!(
			started.elapsed() < READ_BOUND * 3,
			"the walk took {:?}",
			started.elapsed()
		);

		tokio::time::timeout(READ_BOUND, async {
			while std::fs::metadata(&snapshot)
				.and_then(|meta| meta.modified())
				.map_or(true, |saved| saved <= saved_before)
			{
				tokio::time::sleep(Duration::from_millis(100)).await;
			}
		})
		.await
		.expect("the walk saved its snapshot");
		let store = cache.store_for(&root).await.expect("store");
		assert!(!store.walk_unfinished().await, "the walk finished");
		assert!(store.contains_path(&root.join("Music/newer.txt")).await);
		assert!(
			store.contains_path(&prompted.join("secret.txt")).await,
			"what the store held under the waiting folder is kept"
		);
		assert_eq!(
			cache.awaiting_access_under(&root),
			vec![prompted.clone()],
			"the held folder waits on permission, and a counted one is not reported"
		);
		assert!(
			cache.access_blocked(&counted),
			"the count keeps its one blocked read rather than opening another"
		);

		drop(answered);
		drop(store);
		drop(library);
		tokio::time::timeout(Duration::from_secs(120), core.shutdown())
			.await
			.expect("shutdown finished")
			.expect("shutdown");
	}

	/// A directory a rule now excludes, such as `~/Library` held from a walk
	/// before `Users/*/Library` existed: the reconcile adds nothing under it,
	/// takes out what is already there, and a later walk does not bring it
	/// back. A source walk applies rules, so it never sweeps the store.
	#[tokio::test(flavor = "multi_thread")]
	async fn an_excluded_directory_gains_nothing_and_loses_what_it_held() {
		let data = tempfile::tempdir().expect("data dir");
		let files = tempfile::tempdir().expect("files dir");
		let root = files.path().canonicalize().expect("root");
		std::fs::create_dir(root.join(".git")).expect("git dir");
		std::fs::write(root.join(".gitignore"), "").expect("gitignore");
		std::fs::create_dir_all(root.join("cache/deep")).expect("mkdir");
		std::fs::write(root.join("cache/old.txt"), "old").expect("write");
		std::fs::write(root.join("cache/deep/older.txt"), "older").expect("write");
		std::fs::write(root.join("keep.txt"), "keep").expect("write");

		let core = crate::Core::new(data.path().to_path_buf())
			.await
			.expect("core");
		let library = core
			.libraries
			.create_library("Excluded", None, core.context.clone())
			.await
			.expect("library");
		let output = TrackSourceAction::from_input(TrackSourceInput {
			path: root.clone(),
			name: None,
			overrides: Default::default(),
		})
		.expect("input")
		.execute(library.clone(), core.context.clone())
		.await
		.expect("track");
		let walk = library
			.jobs()
			.get_job(JobId(output.job_id.expect("tracking dispatched a walk")))
			.await
			.expect("walk job");
		walk.wait().await.expect("walk");

		let store = core
			.context
			.volume_index()
			.store_for(&output.root)
			.await
			.expect("store");
		store.flush().await.expect("flush");
		let cache = output.root.join("cache");
		let held = [cache.join("old.txt"), cache.join("deep/older.txt")];
		let new = cache.join("new.txt");
		for path in &held {
			assert!(store.contains_path(path).await, "walked before the rule");
		}

		std::fs::write(root.join(".gitignore"), "cache/\n").expect("gitignore");
		std::fs::write(&new, "new").expect("write");
		// The watcher is live and hears both writes. It must not take the new
		// file either, so a late arrival cannot slip past the assertions.
		tokio::time::sleep(Duration::from_secs(2)).await;
		store.flush().await.expect("flush");
		assert!(
			!store.contains_path(&new).await,
			"the watcher keeps a file under an excluded directory out"
		);

		let report = reconcile_source_with(
			&core.context,
			Some(library.id()),
			&output.root,
			false,
			Pass::default(),
		)
		.await
		.expect("the pass ran");
		store.flush().await.expect("flush");
		assert_eq!(report.excluded, 1, "the held directory is left unread");

		let index = core.context.volume_index().resolve_index(&output.root);
		for path in held.iter().chain([&new]) {
			assert!(
				!store.contains_path(path).await,
				"{} is under an excluded directory",
				path.display()
			);
			assert!(index.read().await.get_entry_uuid(path).is_none());
		}
		assert!(
			index.read().await.get_entry_uuid(&cache).is_some(),
			"the directory itself stays, as a walk keeps it"
		);
		assert!(store.contains_path(&output.root.join("keep.txt")).await);

		let walk = crate::ops::sources::track::action::dispatch_source_walk(
			&library,
			&core.context,
			output.id,
			output.root.clone(),
			output.whole_volume,
			false,
		)
		.await
		.expect("rewalk");
		library
			.jobs()
			.get_job(JobId(walk))
			.await
			.expect("rewalk job")
			.wait()
			.await
			.expect("rewalk");
		store.flush().await.expect("flush");
		for path in held.iter().chain([&new]) {
			assert!(
				!store.contains_path(path).await,
				"the walk does not bring back {}",
				path.display()
			);
		}
		assert!(store.contains_path(&output.root.join("keep.txt")).await);
		drop(index);
		drop(store);
		drop(library);
		core.shutdown().await.expect("shutdown");
	}
}
