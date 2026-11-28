//! # Space versioning
//!
//! Keeps the bytes of every change inside a space that has versioning on,
//! so any earlier version of a file can be read back and restored. The
//! store side is [`sd_store::SpaceVersions`]; this module decides when to
//! capture and writes restores back through the agent write path.
//!
//! Captures follow the space event log. An agent write or an app operation
//! is captured as soon as its event is logged, from the bytes it just put
//! on disk. A change the watcher sees is captured once its path has been
//! quiet for a moment, from whatever the file holds then, so an editor's
//! burst of saves is one version. A change that something overwrote before
//! the capture read it is never stored; the interface says so. A space with
//! versioning off reaches none of this: the flag is checked in the same
//! cached lookup that finds the space, and its versions are never opened.
//!
//! Turning versioning on captures every file already in the space as its
//! baseline, so the first edit has something to go back to. Turning it off
//! deletes the space's versions and snapshots, since freeing that space is
//! the reason to turn it off.

use std::{
	collections::{HashMap, HashSet},
	future::Future,
	path::{Component, Path, PathBuf},
	sync::{Arc, LazyLock},
	time::{Duration, Instant},
};

use chrono::Utc;
use parking_lot::Mutex;
use sd_store::{SpaceEvent, SpaceVersions, VersionRow};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::Space,
	ops::{
		agent_write::{restore_in, state_of, Fill, PathKind, WriteError, WriteOutcome, WriteTurn},
		grants::{GrantCap, GrantScope},
		spaces::events::{self, Attribution, Change, InSpace, Origin, RETENTION_DAYS},
	},
};

/// How long a path the watcher reported must stay quiet before its bytes
/// are captured.
const SETTLE: Duration = Duration::from_millis(1500);

const PRUNE_EVERY: Duration = Duration::from_secs(60 * 60);

/// The origin of a version captured when versioning was turned on.
pub const BASELINE: &str = "baseline";

static OPEN: LazyLock<tokio::sync::Mutex<HashMap<Uuid, (Arc<SpaceVersions>, Instant)>>> =
	LazyLock::new(Default::default);
static SETTLING: LazyLock<Mutex<HashMap<PathBuf, Instant>>> = LazyLock::new(Default::default);

/// The directory a space keeps its event log and versions in.
pub fn space_dir(context: &CoreContext, source_id: Uuid, space_id: Uuid) -> Option<PathBuf> {
	Some(
		context
			.volume_index()
			.store_dir(source_id)?
			.join("spaces")
			.join(space_id.to_string()),
	)
}

/// The versions of a space, opened once and pruned hourly while in use.
/// Callers check that the space has versioning on first, since this creates
/// the files.
pub async fn open(
	context: &CoreContext,
	source_id: Uuid,
	space_id: Uuid,
) -> Option<Arc<SpaceVersions>> {
	let mut open = OPEN.lock().await;
	if let Some((versions, pruned)) = open.get_mut(&space_id) {
		if pruned.elapsed() >= PRUNE_EVERY {
			*pruned = Instant::now();
			prune(space_id, versions).await;
		}
		return Some(versions.clone());
	}
	let dir = space_dir(context, source_id, space_id)?;
	let versions = match SpaceVersions::open(&dir).await {
		Ok(versions) => Arc::new(versions),
		Err(error) => {
			warn!(space = %space_id, %error, "could not open a space's versions");
			return None;
		}
	};
	prune(space_id, &versions).await;
	open.insert(space_id, (versions.clone(), Instant::now()));
	Some(versions)
}

/// The versions a space has, whether it keeps versions now (`on`) or was
/// turned off with versions left, which stay readable and restorable until
/// they age out or are purged. None when it has none.
pub async fn kept(
	context: &CoreContext,
	source_id: Uuid,
	space_id: Uuid,
	on: bool,
) -> Option<Arc<SpaceVersions>> {
	if !on && !has_versions(context, source_id, space_id) {
		return None;
	}
	open(context, source_id, space_id).await
}

/// Whether a space has a version store on this device.
pub fn has_versions(context: &CoreContext, source_id: Uuid, space_id: Uuid) -> bool {
	space_dir(context, source_id, space_id)
		.is_some_and(|dir| dir.join(sd_store::SPACE_VERSIONS_FILE).exists())
}

/// Stop capturing for a space whose versioning was turned off. Its versions
/// and snapshots stay on disk.
pub async fn close(space_id: Uuid) {
	if let Some((versions, _)) = OPEN.lock().await.remove(&space_id) {
		versions.close().await;
	}
}

async fn prune(space_id: Uuid, versions: &SpaceVersions) {
	let cutoff = (Utc::now() - chrono::Duration::days(RETENTION_DAYS)).to_rfc3339();
	match versions.prune(&cutoff).await {
		Ok(pruned) if pruned.versions > 0 => {
			debug!(space = %space_id, ?pruned, "pruned versions past retention")
		}
		Ok(_) => {}
		Err(error) => warn!(space = %space_id, %error, "could not prune a space's versions"),
	}
}

/// Follow one logged change into the space's versions: a move or a delete
/// rewrites the manifest, and anything else captures the bytes it left,
/// except a change the watcher saw, which [`settle`] captures.
pub(super) async fn follow(
	context: &CoreContext,
	at: &InSpace,
	event: &SpaceEvent,
	change: &Change,
	who: Attribution,
	seq: Option<i64>,
) {
	if who.origin == Origin::Watcher && !matches!(event.op.as_str(), "move" | "delete") {
		return;
	}
	let Some(versions) = open(context, at.source_id, at.space_id).await else {
		return;
	};
	let template = VersionRow::attributed(&event.at, who.actor, who.origin.as_str(), seq);
	let result = match event.op.as_str() {
		"move" => {
			let to = event.to_path.as_deref().unwrap_or_default();
			versions.moved(&event.path, to, &template).await.map(drop)
		}
		"delete" => versions.removed(&event.path, &template).await.map(drop),
		_ => {
			let landed = change.to.as_deref().unwrap_or(&change.path);
			capture_tree(context, at, &versions, landed, &event.path, &template).await;
			Ok(())
		}
	};
	if let Err(error) = result {
		warn!(space = %at.space_id, path = %event.path, %error, "could not record a version");
	}
}

/// Capture `path` once nothing has touched it for a moment. Called for every
/// change the watcher reports inside a space with versioning on.
pub fn settle(context: &Arc<CoreContext>, path: &Path) {
	let due = Instant::now() + SETTLE;
	if SETTLING.lock().insert(path.to_path_buf(), due).is_some() {
		return;
	}
	let (context, path) = (context.clone(), path.to_path_buf());
	tokio::spawn(async move {
		loop {
			let due = SETTLING.lock().get(&path).copied();
			let Some(due) = due else {
				return;
			};
			let now = Instant::now();
			if due > now {
				tokio::time::sleep(due - now).await;
				continue;
			}
			SETTLING.lock().remove(&path);
			break;
		}
		capture_watched(&context, &path).await;
	});
}

/// Capture what the watcher saw at `path`, attributed to this device and to
/// the event that reported it. A path that is gone is marked deleted, which
/// changes nothing when a logged delete already did.
async fn capture_watched(context: &CoreContext, path: &Path) {
	let Some(at) = events::space_of(context, path, None)
		.await
		.filter(|at| at.versioning)
	else {
		return;
	};
	let gone = match tokio::fs::symlink_metadata(path).await {
		Ok(_) => false,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
		Err(error) => {
			debug!(path = %path.display(), %error, "could not read a changed path; leaving its versions as they are");
			return;
		}
	};
	let Ok(device) = context.device_manager.device_id() else {
		return;
	};
	let Some(versions) = open(context, at.source_id, at.space_id).await else {
		return;
	};
	let seq = match events::log_of(context, at.source_id, at.space_id).await {
		Some(log) => log
			.last_for_path(&at.path)
			.await
			.ok()
			.flatten()
			.map(|event| event.seq),
		None => None,
	};
	let template = VersionRow::attributed(
		&Utc::now().to_rfc3339(),
		device,
		Origin::Watcher.as_str(),
		seq,
	);
	if gone {
		if let Err(error) = versions.removed(&at.path, &template).await {
			warn!(space = %at.space_id, path = %at.path, %error, "could not record a delete");
		}
		return;
	}
	capture_tree(context, &at, &versions, path, &at.path, &template).await;
}

/// Capture the file at `path`, or every file under it, as `relative` and the
/// paths under it. A file that vanished meanwhile is skipped, and so are
/// spaces nested inside, which keep their own versions or none.
async fn capture_tree(
	context: &CoreContext,
	at: &InSpace,
	versions: &SpaceVersions,
	path: &Path,
	relative: &str,
	template: &VersionRow,
) {
	let root = path.to_path_buf();
	let nested = nested_roots(context, at.source_id, at.space_id).await;
	let files = tokio::task::spawn_blocking(move || files_under(&root, &nested))
		.await
		.unwrap_or_default();
	for (file, below) in files {
		let row = VersionRow {
			path: join(relative, &below),
			..template.clone()
		};
		if let Err(error) = versions.capture(&file, row).await {
			debug!(path = %file.display(), %error, "could not capture a version");
		}
	}
}

/// Every regular file at or under `root`, with its path below `root`,
/// `/`-separated. Links are not followed, and folders in `skip` and the ones
/// Spacedrive manages are left out.
fn files_under(root: &Path, skip: &[PathBuf]) -> Vec<(PathBuf, String)> {
	let mut files = Vec::new();
	let mut pending = vec![(root.to_path_buf(), String::new())];
	while let Some((path, below)) = pending.pop() {
		let Ok(meta) = std::fs::symlink_metadata(&path) else {
			continue;
		};
		if meta.is_file() {
			files.push((path, below));
			continue;
		}
		if !meta.is_dir() || skip.contains(&path) || crate::config::is_managed(&path) {
			continue;
		}
		let Ok(entries) = std::fs::read_dir(&path) else {
			continue;
		};
		for entry in entries.flatten() {
			let name = entry.file_name().to_string_lossy().into_owned();
			pending.push((entry.path(), join(&below, &name)));
		}
	}
	files.sort_by(|a, b| a.1.cmp(&b.1));
	files
}

fn join(relative: &str, below: &str) -> String {
	match (relative.is_empty(), below.is_empty()) {
		(_, true) => relative.to_string(),
		(true, false) => below.to_string(),
		(false, false) => format!("{relative}/{below}"),
	}
}

/// The folders of the spaces inside `space_id`, which its captures leave out.
async fn nested_roots(context: &CoreContext, source_id: Uuid, space_id: Uuid) -> Vec<PathBuf> {
	let spaces = events::spaces_of(context, source_id).await;
	let Some(own) = spaces
		.iter()
		.find(|space| space.id == space_id)
		.map(|space| &space.root)
	else {
		return Vec::new();
	};
	let Some(base) = context.volume_index().source_root(source_id) else {
		return Vec::new();
	};
	let inside = format!("{own}/");
	spaces
		.iter()
		.filter(|space| {
			space.id != space_id
				&& !space.root.is_empty()
				&& (own.is_empty() || space.root.starts_with(&inside))
		})
		.map(|space| base.join(&space.root))
		.collect()
}

/// Every file in `space` now, with its path in the space, leaving out the
/// spaces nested inside it.
pub async fn current_files(context: &CoreContext, space: &Space) -> Vec<(PathBuf, String)> {
	let Some(root) = space.path.clone() else {
		return Vec::new();
	};
	let nested = nested_roots(context, space.source_id, space.id).await;
	tokio::task::spawn_blocking(move || files_under(&root, &nested))
		.await
		.unwrap_or_default()
}

/// Capture every file already in a space that just turned versioning on, as
/// its baseline. Folders that are spaces of their own are left to them.
/// Answers how many files were captured.
pub async fn baseline(context: &CoreContext, space: &Space, actor: Uuid) -> usize {
	let Some(root) = space.path.clone() else {
		return 0;
	};
	let Some(versions) = open(context, space.source_id, space.id).await else {
		return 0;
	};
	let nested = nested_roots(context, space.source_id, space.id).await;
	let started = Instant::now();
	let walk_root = root.clone();
	let files = tokio::task::spawn_blocking(move || files_under(&walk_root, &nested))
		.await
		.unwrap_or_default();
	let template = VersionRow::attributed(&Utc::now().to_rfc3339(), actor, BASELINE, None);
	// Versions kept while versioning was off may name files that went away
	// meanwhile. Record those deletes, each confirmed on disk, so a new
	// snapshot does not bring them back.
	let walked: HashSet<&str> = files.iter().map(|(_, below)| below.as_str()).collect();
	for row in versions.manifest(None, None).await.unwrap_or_default() {
		if walked.contains(row.path.as_str()) {
			continue;
		}
		let file = root.join(&row.path);
		let delete = VersionRow {
			path: row.path,
			..template.clone()
		};
		if let Err(error) = versions.remove_if_gone(&file, row.id, &delete).await {
			debug!(path = %delete.path, %error, "could not record a delete in the baseline");
		}
	}
	let mut captured = 0;
	for (file, below) in files {
		if versions.is_closed() {
			break;
		}
		let row = VersionRow {
			path: below,
			..template.clone()
		};
		match versions.capture(&file, row).await {
			Ok(Some(_)) => captured += 1,
			Ok(None) => {}
			Err(error) => debug!(path = %file.display(), %error, "could not capture a baseline"),
		}
	}
	info!(
		space = %space.id,
		files = captured,
		elapsed_ms = started.elapsed().as_millis() as u64,
		"captured a space's baseline versions"
	);
	captured
}

/// Delete every version and snapshot of `space`. A space with versioning
/// on keeps capturing changes from here. Answers whether it had any.
pub async fn purge(context: &CoreContext, space: &Space) -> Result<bool, String> {
	let had = has_versions(context, space.source_id, space.id);
	discard(context, space.source_id, space.id)
		.await
		.map_err(|error| format!("could not delete the space's versions: {error}"))?;
	Ok(had)
}

/// Delete a space's versions and snapshots, for a purge or a removed space.
/// Its event log stays.
pub async fn discard(
	context: &CoreContext,
	source_id: Uuid,
	space_id: Uuid,
) -> std::io::Result<()> {
	// Holding the open handles until the files are gone keeps a re-enable
	// from opening the store while it is being deleted.
	let mut open = OPEN.lock().await;
	if let Some((versions, _)) = open.remove(&space_id) {
		versions.close().await;
	}
	let Some(dir) = space_dir(context, source_id, space_id) else {
		return Ok(());
	};
	let gone = |result: std::io::Result<()>| match result {
		Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
		_ => Ok(()),
	};
	// The database goes first, so a failure part way leaves no history that
	// names chunks already deleted.
	for name in [
		sd_store::SPACE_VERSIONS_FILE.to_string(),
		format!("{}-wal", sd_store::SPACE_VERSIONS_FILE),
		format!("{}-shm", sd_store::SPACE_VERSIONS_FILE),
	] {
		gone(tokio::fs::remove_file(dir.join(name)).await)?;
	}
	gone(tokio::fs::remove_dir_all(dir.join(sd_store::BLOBS_DIR)).await)
}

/// What a restore reads from.
#[derive(Debug, Clone)]
pub enum RestoreFrom {
	/// One version of one file, by id.
	Version(i64),
	/// The files a snapshot pinned, by snapshot id or name.
	Snapshot(String),
}

/// Work out what a restore writes, as (path in the space, content hash).
/// For a version, `path` is where it goes, its own path by default. For a
/// snapshot, `path` narrows it to a file or folder, the whole space by
/// default.
pub async fn plan(
	versions: &SpaceVersions,
	from: &RestoreFrom,
	path: Option<&str>,
) -> Result<Vec<(String, String)>, String> {
	match from {
		RestoreFrom::Version(id) => {
			let row = versions
				.version(*id)
				.await
				.map_err(|e| e.to_string())?
				.ok_or_else(|| format!("no version {id}; it may be past retention"))?;
			let hash = row
				.blob_hash
				.ok_or_else(|| format!("version {id} is a delete; restore an earlier version"))?;
			let target = path
				.map(|path| path.trim_matches('/').to_string())
				.filter(|path| !path.is_empty())
				.unwrap_or(row.path);
			Ok(vec![(target, hash)])
		}
		RestoreFrom::Snapshot(name) => {
			let snapshot = versions
				.snapshot(name)
				.await
				.map_err(|e| e.to_string())?
				.ok_or_else(|| format!("no snapshot {name}"))?;
			let entries = versions
				.manifest(Some(snapshot.manifest_id), path)
				.await
				.map_err(|e| e.to_string())?;
			if entries.is_empty() {
				return Err(format!(
					"snapshot {} holds nothing at {}",
					snapshot.name,
					path.unwrap_or("the space root")
				));
			}
			Ok(entries
				.into_iter()
				.filter_map(|entry| Some((entry.path, entry.blob_hash?)))
				.collect())
		}
	}
}

/// Write the stored content `hash` to the path `scope` names, through the
/// write path, as `who`. Answers None when the file there already holds it.
///
/// The file on disk is compared, not the newest captured version, since a
/// watcher change may not be captured yet. The check runs under the write
/// turn so no other write lands between it and the restore.
pub async fn restore_file(
	context: &CoreContext,
	versions: Arc<SpaceVersions>,
	scope: &GrantScope,
	who: Attribution,
	hash: String,
) -> Result<Option<WriteOutcome>, WriteError> {
	let turn = WriteTurn::take(Default::default()).await;
	// The write path's own reader, which leaves a cloud placeholder unread.
	let live = state_of(context, &scope.root, &scope.path, Default::default()).await?;
	if live.kind == Some(PathKind::File) && live.content_hash.as_deref() == Some(hash.as_str()) {
		return Ok(None);
	}
	let fill = Fill::With(Box::new(move |partial: PathBuf| {
		Box::pin(async move {
			versions
				.read_blob_to(&hash, &partial)
				.await
				.map(drop)
				.map_err(|e| format!("read the stored version: {e}"))
		}) as std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
	}));
	restore_in(context, scope, who, fill, turn).await.map(Some)
}

/// The scope a person's restore writes in: the space's own folder, with
/// `relative` checked to stay inside it.
pub async fn person_scope(
	library_id: Uuid,
	space: &Space,
	actor: Uuid,
	relative: &str,
) -> Result<GrantScope, String> {
	let folder = space
		.path
		.as_ref()
		.ok_or_else(|| "the space's folder is not on this device".to_string())?;
	let root = tokio::fs::canonicalize(folder)
		.await
		.map_err(|e| format!("the space's folder is not reachable: {e}"))?;
	let relative = PathBuf::from(relative);
	if relative.as_os_str().is_empty()
		|| !relative
			.components()
			.all(|part| matches!(part, Component::Normal(_)))
	{
		return Err(format!(
			"{} is not a path inside the space",
			relative.display()
		));
	}
	Ok(GrantScope {
		grant_id: Uuid::nil(),
		actor_id: actor,
		space_id: space.id,
		authority_epoch: 0,
		library_id,
		cap: GrantCap::ReadWrite,
		path: root.join(&relative),
		root,
		relative,
		expires_at: Utc::now() + chrono::Duration::hours(1),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_walk_finds_files_and_leaves_out_nested_spaces_and_links() {
		let dir = tempfile::tempdir().unwrap();
		let root = dir.path();
		std::fs::create_dir_all(root.join("a/b")).unwrap();
		std::fs::create_dir_all(root.join("inner")).unwrap();
		std::fs::write(root.join("top.md"), "1").unwrap();
		std::fs::write(root.join("a/b/deep.md"), "2").unwrap();
		std::fs::write(root.join("inner/own.md"), "3").unwrap();
		#[cfg(unix)]
		std::os::unix::fs::symlink(root.join("top.md"), root.join("link.md")).unwrap();
		let found: Vec<String> = files_under(root, &[root.join("inner")])
			.into_iter()
			.map(|(_, below)| below)
			.collect();
		assert_eq!(found, ["a/b/deep.md", "top.md"]);
		assert_eq!(
			files_under(&root.join("top.md"), &[])
				.into_iter()
				.map(|(_, below)| below)
				.collect::<Vec<_>>(),
			[""]
		);
	}

	#[test]
	fn joined_paths_have_no_stray_slashes() {
		assert_eq!(join("", "a.md"), "a.md");
		assert_eq!(join("docs", ""), "docs");
		assert_eq!(join("docs", "a.md"), "docs/a.md");
	}
}
