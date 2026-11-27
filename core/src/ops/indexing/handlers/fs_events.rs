//! Filesystem event handler for the volume index
//!
//! Subscribes to filesystem events and routes each one under a watched root to
//! the responder, which applies it to the arena and the source store. A change
//! under a summarised directory marks that directory for a debounced recount
//! instead.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use sd_fs_watcher::{FsEvent, FsEventKind};
use tokio::sync::{broadcast, RwLock};

use crate::context::CoreContext;
use crate::infra::job::types::JobPriority;
use crate::ops::indexing::content_identity::ContentIdentityJob;
use crate::ops::indexing::content_text::{self, ContentTextJob};
use crate::ops::indexing::responder;
use crate::ops::indexing::rules::RuleToggles;
use crate::service::watcher::FsWatcherService;
use tracing::{debug, error, trace, warn};

/// How long changes under summarised directories collect before those
/// directories are recounted.
const RECOUNT_INTERVAL: Duration = Duration::from_secs(30);

/// How long changes collect before the hashing nudge looks at their sources.
const HASH_NUDGE_INTERVAL: Duration = Duration::from_secs(30);

/// Least time between two hashing nudges for one source. Under a steady
/// stream of changes, such as a build or a browser cache under home, a nudge
/// every interval started a hashing pass, and the text and photo metadata
/// passes behind it, twice a minute. A source changed after a quiet spell is
/// still nudged within one interval.
const PASS_GAP: Duration = Duration::from_secs(5 * 60);

/// Which sources the hashing nudge owes a pass, and when each last got one.
#[derive(Default)]
struct NudgeSchedule {
	dirty: HashSet<PathBuf>,
	last_nudge: HashMap<PathBuf, Instant>,
}

impl NudgeSchedule {
	fn mark(&mut self, root: PathBuf) {
		self.dirty.insert(root);
	}

	/// Take the dirty sources whose last nudge is at least `PASS_GAP` old.
	/// The others stay dirty, so their changes join the next pass.
	fn take_due(&mut self, now: Instant) -> Vec<PathBuf> {
		let due: Vec<PathBuf> = self
			.dirty
			.iter()
			.filter(|root| {
				self.last_nudge
					.get(*root)
					.is_none_or(|last| now.duration_since(*last) >= PASS_GAP)
			})
			.cloned()
			.collect();
		for root in &due {
			self.dirty.remove(root);
		}
		due
	}

	fn nudged(&mut self, root: PathBuf, now: Instant) {
		self.last_nudge.insert(root, now);
	}
}

/// Handler for filesystem events under watched roots
///
/// Subscribes to `FsWatcher` events and routes matching events to the
/// responder.
pub struct FsEventHandler {
	/// Core context (contains volume_index)
	context: Arc<CoreContext>,
	/// Reference to the filesystem watcher service (set via connect())
	fs_watcher: RwLock<Option<Arc<FsWatcherService>>>,
	/// Whether the handler is running
	is_running: Arc<AtomicBool>,
	/// Default rule toggles for filtering
	rule_toggles: RuleToggles,
}

impl FsEventHandler {
	/// Create a new handler (unconnected)
	///
	/// Call `connect()` to attach to a FsWatcherService before starting.
	pub fn new_unconnected(context: Arc<CoreContext>) -> Self {
		Self {
			context,
			fs_watcher: RwLock::new(None),
			is_running: Arc::new(AtomicBool::new(false)),
			rule_toggles: RuleToggles::default(),
		}
	}

	/// Create a new handler (connected)
	pub fn new(context: Arc<CoreContext>, fs_watcher: Arc<FsWatcherService>) -> Self {
		Self {
			context,
			fs_watcher: RwLock::new(Some(fs_watcher)),
			is_running: Arc::new(AtomicBool::new(false)),
			rule_toggles: RuleToggles::default(),
		}
	}

	/// Connect to a FsWatcherService
	pub async fn connect(&self, fs_watcher: Arc<FsWatcherService>) {
		*self.fs_watcher.write().await = Some(fs_watcher);
	}

	/// Start the event handler
	///
	/// Spawns a task that subscribes to filesystem events and routes
	/// matching events to the responder.
	pub async fn start(&self) -> Result<()> {
		if self.is_running.swap(true, Ordering::SeqCst) {
			warn!("FsEventHandler is already running");
			return Ok(());
		}

		let fs_watcher = self.fs_watcher.read().await.clone();
		let Some(fs_watcher) = fs_watcher else {
			return Err(anyhow::anyhow!(
				"FsEventHandler not connected to FsWatcherService"
			));
		};

		debug!("Starting FsEventHandler");

		let mut rx = fs_watcher.subscribe();
		let context = self.context.clone();
		let rule_toggles = self.rule_toggles;
		let is_running = self.is_running.clone();

		// Summarised directories keep no tree to update, so a change under one
		// marks it for recounting instead. Doing that on a timer is what keeps
		// a file being written in a loop from recounting its whole subtree once
		// per write.
		{
			let context = context.clone();
			let is_running = is_running.clone();
			tokio::spawn(async move {
				while is_running.load(Ordering::SeqCst) {
					tokio::time::sleep(RECOUNT_INTERVAL).await;
					let recounted = responder::recount_summaries(&context).await;
					if recounted > 0 {
						debug!("Recounted {recounted} summarised directories");
					}
				}
			});
		}

		// Sources the watcher has changed files under, drained on a timer.
		// The content job claims its work from the store and dispatch dedupes
		// on the root, so nudging is idempotent; the schedule bounds how often
		// a busy source pays for a pass and the passes chained behind it.
		let schedule = Arc::new(std::sync::Mutex::new(NudgeSchedule::default()));
		{
			let schedule = schedule.clone();
			let context = context.clone();
			let is_running = is_running.clone();
			tokio::spawn(async move {
				while is_running.load(Ordering::SeqCst) {
					tokio::time::sleep(HASH_NUDGE_INTERVAL).await;
					let roots = schedule.lock().unwrap().take_due(Instant::now());
					if roots.is_empty() {
						continue;
					}
					let libraries = context.libraries().await.get_open_libraries().await;
					let Some(library) = libraries.first() else {
						continue;
					};
					for root in roots {
						let job = ContentIdentityJob::background(root.clone());
						// A pass still running claims what changed as it goes.
						// The root stays dirty so anything it missed is picked
						// up once it finishes, and no second row is started.
						if library.jobs().has_live(&job).await
							|| library
								.jobs()
								.has_live(&ContentTextJob::new(root.clone(), true))
								.await
						{
							schedule.lock().unwrap().mark(root);
							continue;
						}
						schedule
							.lock()
							.unwrap()
							.nudged(root.clone(), Instant::now());
						// A dirty root with nothing pending gets no job: the
						// count is one indexed query, while a job is a row, a
						// dispatch, and a progress card that says "0%" and
						// vanishes. A count that fails still dispatches, so
						// the job surfaces the store error instead of the
						// nudge swallowing it.
						let pending = match context.volume_index().store_for(&root).await {
							Some(store) => store.files_needing_content_count().await.unwrap_or(1),
							None => 0,
						};
						if pending == 0 {
							// A rename or a change hashing already knows can
							// still leave text to read, such as a file renamed
							// to an extension the text index reads.
							content_text::index_text_soon(library, &root, true).await;
							continue;
						}
						if let Err(e) = library
							.jobs()
							.dispatch_with_priority(job, JobPriority::LOW, None)
							.await
						{
							warn!(root = %root.display(), "could not nudge hashing: {e}");
						}
					}
				}
			});
		}

		let schedule_for_events = schedule.clone();
		tokio::spawn(async move {
			debug!("FsEventHandler task started");

			while is_running.load(Ordering::SeqCst) {
				match rx.recv().await {
					Ok(event) => {
						// Spacedrive's own writes are not changes to index or
						// hash. Without this a store's SQLite journals dirty
						// the source, the nudged hashing job writes the store,
						// and the loop feeds itself every interval.
						if crate::config::is_managed(&event.path) {
							continue;
						}
						if let Err(e) = Self::handle_event(&context, &event, rule_toggles).await {
							error!("Error handling filesystem event: {}", e);
						} else if let Some(root) =
							context.volume_index().source_root_for(&event.path)
						{
							schedule_for_events.lock().unwrap().mark(root);
						}
					}
					Err(broadcast::error::RecvError::Lagged(n)) => {
						warn!("FsEventHandler lagged by {} events", n);
						// Continue processing - we'll catch up
					}
					Err(broadcast::error::RecvError::Closed) => {
						debug!("FsWatcher channel closed, stopping FsEventHandler");
						break;
					}
				}
			}

			debug!("FsEventHandler task stopped");
		});

		Ok(())
	}

	/// Stop the event handler
	pub fn stop(&self) {
		debug!("Stopping FsEventHandler");
		self.is_running.store(false, Ordering::SeqCst);
	}

	/// Check if the handler is running
	pub fn is_running(&self) -> bool {
		self.is_running.load(Ordering::SeqCst)
	}

	/// Handle a single filesystem event
	async fn handle_event(
		context: &Arc<CoreContext>,
		event: &FsEvent,
		rule_toggles: RuleToggles,
	) -> Result<()> {
		let index = context.volume_index();
		let (root_path, event) = match index.watched_root_for_change(&event.path).await {
			Some(root) => (root, event.clone()),
			None => {
				// A move into a directory the index does not hold, such as a
				// trash folder, still takes the file away from where it was.
				let from = match &event.kind {
					FsEventKind::Rename { from, .. } => from,
					_ => {
						trace!("Event not under a watched root: {}", event.path.display());
						return Ok(());
					}
				};
				let Some(root) = index.watched_root_for_change(from).await else {
					trace!("Rename outside every watched root: {}", from.display());
					return Ok(());
				};
				debug!(
					"Rename into unindexed {}, removing {}",
					event.path.display(),
					from.display()
				);
				(root, FsEvent::remove(from.clone()))
			}
		};
		let root_path = &root_path;

		debug!(
			"Event matched: {} (root: {})",
			event.path.display(),
			root_path.display()
		);

		// A vanished watched root means the volume unmounted; events from the
		// flood that follows must not mutate the in-memory index.
		if !super::root_is_present(root_path) {
			warn!(
				"Watched root {} missing, dropping {:?} event for {} — volume likely unmounted",
				root_path.display(),
				event.kind,
				event.path.display()
			);
			return Ok(());
		}

		// The source's own capture policy, not the handler's default: an
		// archival source keeps everything its walk keeps, live changes
		// included.
		let rule_toggles = context.volume_index().rule_toggles_for(root_path);
		responder::apply(context, root_path, event, rule_toggles).await
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Replays the schedule against a clock instead of waiting on one.
	fn run(changes_at: impl Fn(Duration) -> bool, for_how_long: Duration) -> Vec<Duration> {
		let start = Instant::now();
		let root = PathBuf::from("/home/someone");
		let mut schedule = NudgeSchedule::default();
		let mut nudges = Vec::new();
		let mut elapsed = Duration::ZERO;
		while elapsed < for_how_long {
			if changes_at(elapsed) {
				schedule.mark(root.clone());
			}
			elapsed += HASH_NUDGE_INTERVAL;
			let now = start + elapsed;
			for root in schedule.take_due(now) {
				schedule.nudged(root, now);
				nudges.push(elapsed);
			}
		}
		nudges
	}

	#[test]
	fn a_steady_stream_of_changes_gets_one_pass_per_gap() {
		let hour = Duration::from_secs(3600);
		let nudges = run(|_| true, hour);
		assert_eq!(nudges.len() as u64, hour.as_secs() / PASS_GAP.as_secs());
		for pair in nudges.windows(2) {
			assert!(pair[1] - pair[0] >= PASS_GAP);
		}
	}

	#[test]
	fn a_change_after_a_quiet_spell_is_nudged_within_one_interval() {
		let nudges = run(
			|at| at == Duration::from_secs(600),
			Duration::from_secs(900),
		);
		assert_eq!(nudges, vec![Duration::from_secs(600) + HASH_NUDGE_INTERVAL]);
	}

	#[test]
	fn changes_inside_the_gap_wait_for_it_instead_of_being_dropped() {
		let nudges = run(
			|at| at == Duration::ZERO || at == Duration::from_secs(60),
			Duration::from_secs(900),
		);
		assert_eq!(
			nudges,
			vec![HASH_NUDGE_INTERVAL, HASH_NUDGE_INTERVAL + PASS_GAP]
		);
	}

	#[test]
	fn a_deferred_source_stays_dirty() {
		let root = PathBuf::from("/home/someone");
		let mut schedule = NudgeSchedule::default();
		let now = Instant::now();
		schedule.mark(root.clone());
		let due = schedule.take_due(now);
		assert_eq!(due, vec![root.clone()]);
		// A live pass held the root, so the tick put it back without a nudge.
		schedule.mark(root.clone());
		assert_eq!(schedule.take_due(now + HASH_NUDGE_INTERVAL), vec![root]);
	}
}
