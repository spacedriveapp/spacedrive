//! # Space events
//!
//! Writes each change inside a space to that space's event log, attributed to
//! whoever made it. Three writers feed it: agent writes, file operations run
//! through the app, and the watcher for everything else. Only paths inside a
//! space are logged. A change anywhere else in a source costs one in-memory
//! lookup and writes nothing, so a home folder full of caches and build
//! output never fills a log or starts a job.
//!
//! An agent write or an app operation is seen again by the watcher a moment
//! later. The first writer marks the path, and the watcher skips a marked path
//! for a few seconds, so one change is one event with the right actor.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicU64, Ordering},
		Arc, LazyLock,
	},
	time::{Duration, Instant},
};

use chrono::Utc;
use globset::GlobSet;
use parking_lot::Mutex;
use sd_fs_watcher::{FsEvent, FsEventKind};
use sd_store::{SpaceEvent, SpaceEventLog};
use tracing::warn;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	infra::job::journal::Effect,
	ops::{grants::protect, spaces::store::slash_path},
};

/// How long a space keeps its events unless it says otherwise.
pub const RETENTION_DAYS: i64 = 30;

/// How long the watcher leaves a path alone after Spacedrive changed it.
const OWN_WRITE_WINDOW: Duration = Duration::from_secs(5);

/// Repeats of one watcher change within this window are one event: an editor
/// saving a file fires several modify events.
const WATCHER_COALESCE: Duration = Duration::from_secs(2);

const PRUNE_EVERY: Duration = Duration::from_secs(60 * 60);

/// Who made a change and through what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
	AgentWrite,
	AppOp,
	Watcher,
}

impl Origin {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::AgentWrite => "agent_write",
			Self::AppOp => "app_op",
			Self::Watcher => "watcher",
		}
	}
}

/// The attribution every event of one change shares.
#[derive(Debug, Clone, Copy)]
pub struct Attribution {
	pub origin: Origin,
	pub actor: Uuid,
	pub grant: Option<Uuid>,
	/// The job id `files.undo` takes to reverse the change.
	pub undo: Option<Uuid>,
	/// The library the change was made through. A path can sit in sources
	/// of several libraries; only this one's spaces record it. None for the
	/// watcher, which sees the disk rather than a library.
	pub library: Option<Uuid>,
}

/// One change to record, in absolute paths.
#[derive(Debug, Clone)]
pub struct Change {
	pub op: &'static str,
	pub path: PathBuf,
	pub to: Option<PathBuf>,
	pub before_hash: Option<String>,
	pub after_hash: Option<String>,
	pub size: Option<u64>,
}

/// A path resolved to the space that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InSpace {
	pub source_id: Uuid,
	pub space_id: Uuid,
	/// Relative to the space root, `/`-separated. Empty is the root itself.
	pub path: String,
}

struct Logs {
	/// By space: the log, the source whose catalog holds it, and when it
	/// was last pruned.
	open: HashMap<Uuid, (Arc<SpaceEventLog>, Uuid, Instant)>,
}

/// A space as the lookups here need it.
struct Cached {
	id: Uuid,
	root: String,
	/// Its protected patterns, when it has any that compile.
	protected: Option<Arc<GlobSet>>,
}

static SPACES: LazyLock<Mutex<HashMap<Uuid, Arc<Vec<Cached>>>>> = LazyLock::new(Default::default);
static LOGS: LazyLock<tokio::sync::Mutex<Logs>> = LazyLock::new(|| {
	tokio::sync::Mutex::new(Logs {
		open: HashMap::new(),
	})
});
/// A path Spacedrive is changing: how many operations hold it now, and when
/// the last one claimed or let go of it.
struct Claim {
	held: usize,
	at: Instant,
}

static OWN_WRITES: LazyLock<Mutex<HashMap<PathBuf, Claim>>> = LazyLock::new(Default::default);
static WATCHED: LazyLock<Mutex<HashMap<PathBuf, (&'static str, Instant)>>> =
	LazyLock::new(Default::default);

/// Bumped by every invalidation, so a cache fill that read the rows before
/// a space changed does not publish them after.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Close the event logs of a source's spaces and forget its spaces, once the
/// source is retired, so deleting its catalog does not race a log that is
/// still open inside it.
pub async fn forget_source(source_id: Uuid) {
	invalidate(source_id);
	let closing: Vec<_> = {
		let mut logs = LOGS.lock().await;
		let spaces: Vec<Uuid> = logs
			.open
			.iter()
			.filter(|(_, (_, source, _))| *source == source_id)
			.map(|(space, _)| *space)
			.collect();
		spaces
			.into_iter()
			.filter_map(|space| logs.open.remove(&space))
			.collect()
	};
	for (log, _, _) in closing {
		log.close().await;
	}
}

/// Forget what is cached about a source's spaces, after one changed.
pub fn invalidate(source_id: Uuid) {
	let mut spaces = SPACES.lock();
	GENERATION.fetch_add(1, Ordering::SeqCst);
	spaces.remove(&source_id);
}

/// Mark paths Spacedrive is about to change, so the watcher does not record
/// the change a second time as made outside Spacedrive. Called right before
/// the change, because the watcher can see it before the writer records it.
/// A claim covers the path and everything inside it, for a few seconds.
pub fn claim(paths: &[&Path]) {
	let now = Instant::now();
	let mut own = OWN_WRITES.lock();
	if own.len() > 4096 {
		own.retain(|_, claim| claim.held > 0 || now.duration_since(claim.at) < OWN_WRITE_WINDOW);
	}
	for path in paths {
		own.entry(path.to_path_buf())
			.and_modify(|claim| claim.at = now)
			.or_insert(Claim { held: 0, at: now });
	}
}

/// Claim the folders that creating `dir` and its missing parents will add,
/// and nothing that already exists: a claim covers what is inside it, and
/// an existing folder holds files nobody here is touching.
pub async fn claim_missing(dir: &Path) {
	let mut topmost = None;
	for ancestor in dir.ancestors() {
		if tokio::fs::symlink_metadata(ancestor).await.is_ok() {
			break;
		}
		topmost = Some(ancestor);
	}
	if let Some(topmost) = topmost {
		claim(&[topmost]);
	}
}

/// Claim paths for as long as the answer lives, and a few seconds after: for
/// an operation that takes longer than the claim window, such as copying a
/// folder.
pub fn hold(paths: &[&Path]) -> Held {
	let now = Instant::now();
	let mut own = OWN_WRITES.lock();
	for path in paths {
		let claim = own
			.entry(path.to_path_buf())
			.or_insert(Claim { held: 0, at: now });
		claim.held += 1;
		claim.at = now;
	}
	Held {
		paths: paths.iter().map(|path| path.to_path_buf()).collect(),
	}
}

/// Paths held by [`hold`] until this is dropped.
#[must_use = "the paths are released when this is dropped"]
pub struct Held {
	paths: Vec<PathBuf>,
}

impl Drop for Held {
	fn drop(&mut self) {
		let now = Instant::now();
		let mut own = OWN_WRITES.lock();
		for path in &self.paths {
			if let Some(claim) = own.get_mut(path) {
				claim.held = claim.held.saturating_sub(1);
				claim.at = now;
			}
		}
	}
}

/// Whether Spacedrive claimed `path` or a folder holding it.
fn is_own(own: &HashMap<PathBuf, Claim>, path: &Path, now: Instant) -> bool {
	path.ancestors().any(|path| {
		own.get(path)
			.is_some_and(|claim| claim.held > 0 || now.duration_since(claim.at) < OWN_WRITE_WINDOW)
	})
}

async fn spaces_of(context: &CoreContext, source_id: Uuid) -> Arc<Vec<Cached>> {
	let generation = {
		let spaces = SPACES.lock();
		if let Some(spaces) = spaces.get(&source_id) {
			return spaces.clone();
		}
		GENERATION.load(Ordering::SeqCst)
	};
	let spaces = match context.volume_index().read_store(source_id).await {
		Some(db) => match db.spaces().await {
			Ok(rows) => rows.into_iter().map(cached).collect(),
			Err(error) => {
				warn!(%source_id, %error, "could not read the spaces of a source");
				return Arc::new(Vec::new());
			}
		},
		None => Vec::new(),
	};
	let spaces = Arc::new(spaces);
	let mut cache = SPACES.lock();
	if GENERATION.load(Ordering::SeqCst) == generation {
		cache.insert(source_id, spaces.clone());
	}
	spaces
}

fn cached(row: sd_store::SpaceRow) -> Cached {
	let patterns: Vec<String> = serde_json::from_str(&row.protected).unwrap_or_default();
	// A pattern synced from a newer build may not compile here; the space
	// keeps the built-in protection rather than losing its log.
	let protected = match protect::compile(&patterns) {
		Ok(set) if !patterns.is_empty() => Some(Arc::new(set)),
		Ok(_) => None,
		Err(error) => {
			warn!(space = %row.uuid, %error, "ignoring the protected patterns of a space");
			None
		}
	};
	Cached {
		id: row.uuid,
		root: row.root,
		protected,
	}
}

/// `relative` under `root`, when `root` holds it.
fn inside<'a>(root: &str, relative: &'a str) -> Option<&'a str> {
	if root.is_empty() {
		Some(relative)
	} else if relative == root {
		Some("")
	} else {
		relative
			.strip_prefix(root)
			.and_then(|rest| rest.strip_prefix('/'))
	}
}

/// The innermost space of `spaces` that holds `relative`, and `relative`
/// under that space's root.
fn innermost(spaces: &[Cached], relative: &str) -> Option<(Uuid, String)> {
	spaces
		.iter()
		.filter_map(|space| {
			let inside = inside(&space.root, relative)?;
			Some((space.root.len(), space.id, inside.to_string()))
		})
		.max_by_key(|(depth, _, _)| *depth)
		.map(|(_, id, inside)| (id, inside))
}

/// The source of `path`, its spaces, and `path` relative to the source
/// root.
async fn locate(
	context: &CoreContext,
	path: &Path,
	library: Option<Uuid>,
) -> Option<(Uuid, Arc<Vec<Cached>>, String)> {
	let index = context.volume_index();
	let source_for = |path: &Path| match library {
		None => index.source_id_for(path),
		Some(library) => index
			.sources_of(library)
			.into_iter()
			.filter(|source| source.attached && path.starts_with(&source.root))
			.max_by_key(|source| source.root.components().count())
			.map(|source| source.id),
	};
	let spelled;
	let (source_id, path) = match source_for(path) {
		Some(id) => (id, path),
		None => {
			spelled = context.volume_manager.locate_path(path).await?.1;
			(source_for(&spelled)?, spelled.as_path())
		}
	};
	let spaces = spaces_of(context, source_id).await;
	if spaces.is_empty() {
		return None;
	}
	let root = index.source_root(source_id)?;
	let relative = slash_path(path.strip_prefix(&root).ok()?);
	Some((source_id, spaces, relative))
}

/// The space holding `path`, if any. `path` may be spelled under the
/// source's root or under the volume's spelling of it. With `library`, only
/// that library's sources are considered, since sources of two libraries
/// can nest.
pub async fn space_of(
	context: &CoreContext,
	path: &Path,
	library: Option<Uuid>,
) -> Option<InSpace> {
	let (source_id, spaces, relative) = locate(context, path, library).await?;
	let (space_id, path) = innermost(&spaces, &relative)?;
	Some(InSpace {
		source_id,
		space_id,
		path,
	})
}

/// The roots of the attached sources of `library`, and the folder and
/// protected patterns of every space in them that protects anything. Nested
/// sources each hold their own spaces, so a lookup by the one source holding
/// a path would miss the spaces of the sources around it.
pub async fn protected_spaces(
	context: &CoreContext,
	library: Uuid,
) -> (Vec<PathBuf>, Vec<(PathBuf, Arc<GlobSet>)>) {
	let mut roots = Vec::new();
	let mut found = Vec::new();
	for source in context.volume_index().sources_of(library) {
		if !source.attached {
			continue;
		}
		for space in spaces_of(context, source.id).await.iter() {
			if let Some(set) = &space.protected {
				let dir = if space.root.is_empty() {
					source.root.clone()
				} else {
					source.root.join(&space.root)
				};
				found.push((dir, set.clone()));
			}
		}
		roots.push(source.root);
	}
	(roots, found)
}

async fn log_for(context: &CoreContext, at: &InSpace) -> Option<Arc<SpaceEventLog>> {
	let mut logs = LOGS.lock().await;
	if let Some((log, _, pruned)) = logs.open.get_mut(&at.space_id) {
		if pruned.elapsed() >= PRUNE_EVERY {
			*pruned = Instant::now();
			prune(log).await;
		}
		return Some(log.clone());
	}
	let dir = context
		.volume_index()
		.store_dir(at.source_id)?
		.join("spaces")
		.join(at.space_id.to_string());
	let log = match SpaceEventLog::open(&dir).await {
		Ok(log) => Arc::new(log),
		Err(error) => {
			warn!(space = %at.space_id, %error, "could not open a space's event log");
			return None;
		}
	};
	prune(&log).await;
	logs.open
		.insert(at.space_id, (log.clone(), at.source_id, Instant::now()));
	Some(log)
}

async fn prune(log: &SpaceEventLog) {
	let cutoff = (Utc::now() - chrono::Duration::days(RETENTION_DAYS)).to_rfc3339();
	if let Err(error) = log.prune_before(&cutoff).await {
		warn!(%error, "could not prune a space's event log");
	}
}

/// The event log of one space, if its source has a store on this device.
pub async fn log_of(
	context: &CoreContext,
	source_id: Uuid,
	space_id: Uuid,
) -> Option<Arc<SpaceEventLog>> {
	log_for(
		context,
		&InSpace {
			source_id,
			space_id,
			path: String::new(),
		},
	)
	.await
}

/// Record one change in the spaces it touches. A move within a space is one
/// event; a move between spaces, or in or out of one, is a delete where it
/// left and a create where it landed. Answers how many events were written.
pub async fn record(context: &CoreContext, who: Attribution, change: Change) -> usize {
	if who.origin != Origin::Watcher {
		let mut paths = vec![change.path.as_path()];
		paths.extend(change.to.as_deref());
		claim(&paths);
	}

	let from = space_of(context, &change.path, who.library).await;
	let to = match &change.to {
		Some(to) => space_of(context, to, who.library).await,
		None => None,
	};
	let mut written = 0;
	let mut write = |at: InSpace, op: &'static str, to_path: Option<String>| {
		let event = SpaceEvent {
			seq: 0,
			space_id: at.space_id,
			at: Utc::now().to_rfc3339(),
			actor_id: who.actor,
			grant_id: who.grant,
			origin: who.origin.as_str().to_string(),
			op: op.to_string(),
			path: at.path.clone(),
			to_path,
			before_hash: change.before_hash.clone(),
			after_hash: change.after_hash.clone(),
			size: change.size.map(|size| size as i64),
			undo_id: who.undo,
			authority_epoch: 0,
		};
		written += 1;
		(at, event)
	};
	let events = match (from, to) {
		(Some(from), Some(to)) if change.op == "move" && from.space_id == to.space_id => {
			let to_path = Some(to.path);
			vec![write(from, "move", to_path)]
		}
		(from, to) if change.op == "move" => from
			.map(|from| write(from, "delete", None))
			.into_iter()
			.chain(to.map(|to| write(to, "create", None)))
			.collect(),
		(Some(from), _) => vec![write(from, change.op, None)],
		(None, _) => Vec::new(),
	};
	for (at, event) in events {
		let Some(log) = log_for(context, &at).await else {
			continue;
		};
		if let Err(error) = log.append(&event).await {
			warn!(space = %at.space_id, %error, "could not append a space event");
		}
	}
	written
}

/// Record what a journaled operation did, one event per effect.
/// `hashes` names the path the operation was about and its content hashes
/// before and after, where the caller knows them.
pub async fn record_effects(
	context: &CoreContext,
	who: Attribution,
	effects: &[Effect],
	hashes: Option<(&Path, Option<String>, Option<String>)>,
) {
	for effect in effects {
		let Some(mut change) = change_of(effect) else {
			continue;
		};
		if let Some((path, before, after)) = &hashes {
			if change.to.as_deref().unwrap_or(&change.path) == *path {
				change.before_hash = before.clone();
				change.after_hash = after.clone();
			}
		}
		record(context, who, change).await;
	}
}

fn change_of(effect: &Effect) -> Option<Change> {
	let change = |op, path: &PathBuf, to: Option<&PathBuf>, size: Option<u64>| Change {
		op,
		path: path.clone(),
		to: to.cloned(),
		before_hash: None,
		after_hash: None,
		size,
	};
	Some(match effect {
		Effect::Created { path, subject } => {
			let is_dir = subject.as_ref().is_some_and(|s| s.is_dir);
			let op = if is_dir { "mkdir" } else { "create" };
			change(
				op,
				path,
				None,
				subject.as_ref().filter(|s| !s.is_dir).map(|s| s.size),
			)
		}
		Effect::Moved { from, to, subject } => change(
			"move",
			from,
			Some(to),
			subject.as_ref().filter(|s| !s.is_dir).map(|s| s.size),
		),
		Effect::Trashed { from, .. } => change("delete", from, None, None),
		Effect::Removed { path } => change("delete", path, None, None),
		Effect::Replaced { path, subject, .. } => {
			change("modify", path, None, subject.as_ref().map(|s| s.size))
		}
		Effect::Attributes { path, .. } => change("modify", path, None, None),
	})
}

/// Record a change the watcher saw, when it is inside a space and Spacedrive
/// did not make it. It is attributed to this device.
pub async fn record_watched(context: &CoreContext, event: &FsEvent) {
	let (op, path, to) = match &event.kind {
		FsEventKind::Create => ("create", &event.path, None),
		FsEventKind::Modify => ("modify", &event.path, None),
		FsEventKind::Remove => ("delete", &event.path, None),
		FsEventKind::Rename { from, to } => ("move", from, Some(to)),
	};
	let now = Instant::now();
	{
		let own = OWN_WRITES.lock();
		if is_own(&own, path, now) || to.is_some_and(|to| is_own(&own, to, now)) {
			return;
		}
	}
	{
		let mut watched = WATCHED.lock();
		if watched.len() > 4096 {
			watched.retain(|_, (_, at)| now.duration_since(*at) < WATCHER_COALESCE);
		}
		if let Some((last, at)) = watched.get(path) {
			if *last == op && now.duration_since(*at) < WATCHER_COALESCE {
				return;
			}
		}
		watched.insert(path.clone(), (op, now));
	}
	if space_of(context, path, None).await.is_none()
		&& match to {
			Some(to) => space_of(context, to, None).await.is_none(),
			None => true,
		} {
		return;
	}
	let Ok(device) = context.device_manager.device_id() else {
		return;
	};
	let landed = to.unwrap_or(path);
	let meta = tokio::fs::symlink_metadata(landed).await.ok();
	let op = match (op, &meta) {
		("create", Some(meta)) if meta.is_dir() => "mkdir",
		_ => op,
	};
	record(
		context,
		Attribution {
			origin: Origin::Watcher,
			actor: device,
			grant: None,
			undo: None,
			library: None,
		},
		Change {
			op,
			path: path.clone(),
			to: to.cloned(),
			before_hash: None,
			after_hash: None,
			size: meta.filter(|m| m.is_file()).map(|m| m.len()),
		},
	)
	.await;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn creating_folders_claims_only_the_new_ones() {
		let dir = tempfile::tempdir().unwrap();
		let existing = dir.path().join("project");
		std::fs::create_dir(&existing).unwrap();
		claim_missing(&existing.join("new/deeper")).await;
		let own = OWN_WRITES.lock();
		let now = Instant::now();
		assert!(is_own(&own, &existing.join("new/deeper/file.md"), now));
		assert!(
			!is_own(&own, &existing.join("unrelated.md"), now),
			"a folder that was already there stays watched"
		);
	}

	#[test]
	fn the_innermost_space_holds_a_path() {
		let a = Uuid::from_u128(1);
		let b = Uuid::from_u128(2);
		let c = Uuid::from_u128(3);
		let space = |id, root: &str| Cached {
			id,
			root: root.into(),
			protected: None,
		};
		let spaces = vec![
			space(a, ""),
			space(b, "Documents"),
			space(c, "Documents/thesis"),
		];
		assert_eq!(
			innermost(&spaces, "Documents/thesis/ch1.md"),
			Some((c, "ch1.md".into()))
		);
		assert_eq!(
			innermost(&spaces, "Documents/notes.md"),
			Some((b, "notes.md".into()))
		);
		assert_eq!(innermost(&spaces, "Documents"), Some((b, String::new())));
		assert_eq!(
			innermost(&spaces, "Documentsx/a"),
			Some((a, "Documentsx/a".into()))
		);
		assert_eq!(innermost(&spaces[1..], "Library/Caches/x"), None);
	}
}
