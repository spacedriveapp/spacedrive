//! The four writes and the stat that feeds their preconditions.
//!
//! Every write takes one process-wide lock from its grant check to its
//! journal row, so two agents on this daemon cannot interleave a check with
//! a commit. Writers outside Spacedrive are not stopped by it; they are
//! caught by checking the target again just before the rename.

use std::{
	io::{Seek, SeekFrom, Write},
	path::{Path, PathBuf},
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use base64::Engine;
use tokio::sync::{Mutex, MutexGuard};
use tracing::{info, warn};
use uuid::Uuid;

use super::{
	bucket::{self, Bucket},
	conflict::conflict_name,
	state::{self, check, stale},
	ContentEncoding, DeleteInput, MakeDirInput, MoveInput, PathKind, PathState, StatInput,
	WriteError, WriteFileInput, WriteOutcome, MAX_WRITE_BYTES,
};
use crate::{
	context::CoreContext,
	infra::job::{
		journal::{Effect, Subject},
		types::JobId,
	},
	ops::{
		files::trash,
		grants::{
			authorize, bucket::BucketUrl, protect::Protection, scope, GrantCap, GrantError,
			GrantScope,
		},
		spaces::events::{self as space_events, Attribution, Origin},
	},
	service::network::protocol::staged_receive::{rename_noreplace, StagedReceive, JOURNAL_DIR},
};

static WRITES: Mutex<()> = Mutex::const_new(());

/// One agent write's turn: the process-wide write lock, and the flag that
/// cancels the write while nothing has changed yet.
///
/// A surface that gives up on a call, as the agent endpoint does on a
/// timeout or a revoked grant, sets the flag. Hashing stops, and the write
/// refuses at its first change. Once it has changed something it runs to its
/// commit or rollback and its journal row, so the surface runs it as its own
/// task and waits for the turn before spawning it.
pub struct WriteTurn {
	_one_writer: MutexGuard<'static, ()>,
	cancel: Arc<AtomicBool>,
}

impl WriteTurn {
	/// Wait for the write lock. Dropping the future while it waits leaves the
	/// queue.
	pub async fn take(cancel: Arc<AtomicBool>) -> Self {
		Self {
			_one_writer: WRITES.lock().await,
			cancel,
		}
	}

	async fn own() -> Self {
		Self::take(Arc::new(AtomicBool::new(false))).await
	}

	/// The flag a surface sets when it gives up on the call.
	pub(super) fn cancel(&self) -> &AtomicBool {
		&self.cancel
	}

	/// Refuse if the caller gave up. Called just before a write's first change.
	pub(super) fn proceed(&self) -> Result<(), WriteError> {
		if self.cancel.load(Ordering::Relaxed) {
			return Err(WriteError::Cancelled);
		}
		Ok(())
	}
}

/// A path's state: the values a precondition names.
pub async fn stat(context: &CoreContext, input: StatInput) -> Result<PathState, WriteError> {
	let scope = authorize(
		context,
		&input.token,
		"agent.stat",
		&input.path,
		GrantCap::Read,
	)
	.await?;
	if let Some(bucket) = Bucket::of(context, &scope).await? {
		return bucket::state(&bucket, &AtomicBool::new(false)).await;
	}
	Ok(state::read(context, &scope.root, &scope.path).await?.0)
}

/// Write a whole file, or a byte range of an existing one.
///
/// Without `expect` this is a create: it never replaces anything, and a
/// taken name sends the file to a conflict name beside it. With `expect`
/// the file must exist and match, and its previous bytes go to the trash.
pub async fn write_file(
	context: &CoreContext,
	input: WriteFileInput,
) -> Result<WriteOutcome, WriteError> {
	write_file_in(context, input, WriteTurn::own().await).await
}

/// [`write_file`] in a turn the caller already holds.
pub async fn write_file_in(
	context: &CoreContext,
	input: WriteFileInput,
	turn: WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let bytes = match input.encoding.unwrap_or_default() {
		ContentEncoding::Utf8 => input.content.into_bytes(),
		ContentEncoding::Base64 => base64::engine::general_purpose::STANDARD
			.decode(input.content.as_bytes())
			.map_err(|e| WriteError::Invalid(format!("content is not base64: {e}")))?,
	};
	if bytes.len() > MAX_WRITE_BYTES {
		return Err(WriteError::Invalid(format!(
			"one write carries at most {} MiB",
			MAX_WRITE_BYTES / (1024 * 1024)
		)));
	}
	if input.offset.is_some() && input.expect.is_none() {
		return Err(WriteError::Invalid(
			"a byte-range write changes an existing file; name what you read in expect".to_string(),
		));
	}

	let scope = writable(context, &input.token, "agent.write_file", &input.path).await?;
	if let Some(bucket) = Bucket::of(context, &scope).await? {
		let outcome = bucket::write_file(bucket, bytes, input.offset, input.expect, &turn).await;
		relist(context, scope.library_id, &scope.path).await;
		return outcome;
	}
	let (current, metadata) =
		state::read_until(context, &scope.root, &scope.path, turn.cancel.clone()).await?;
	let id = Uuid::now_v7();
	let mut effects = Vec::new();

	let (landed, conflict) = match &input.expect {
		None => {
			let wanted = if current.exists() {
				conflict_name(&scope.path, false)
			} else {
				scope.path.clone()
			};
			turn.proceed()?;
			effects.extend(make_parents(&scope.root, &wanted).await?);
			let mut staged = stage(context, &wanted).await?;
			write_bytes(staged.path(), &bytes, None).await?;
			still_inside(&scope).await?;
			let landed = commit(&mut staged, wanted).await?;
			let conflict = landed != scope.path;
			let meta = std::fs::symlink_metadata(&landed).ok();
			effects.push(Effect::created(landed.clone(), meta.as_ref()));
			(landed, conflict)
		}
		Some(expect) => {
			check(expect, &current)?;
			match current.kind {
				Some(PathKind::File) => {}
				_ => {
					return Err(WriteError::Invalid(format!(
						"{} is not a regular file",
						current.path
					)))
				}
			}
			let metadata = metadata.ok_or_else(|| stale(current.clone()))?;
			if let Some(offset) = input.offset {
				if current.content_hash.is_none() {
					return Err(WriteError::Placeholder(current.path));
				}
				if offset > metadata.len() {
					return Err(WriteError::Invalid(format!(
						"offset {offset} is past the end of {} ({} bytes)",
						current.path,
						metadata.len()
					)));
				}
			}
			let subject = Subject::of(&metadata);
			let mut staged = stage(context, &scope.path).await?;
			let base = input.offset.map(|offset| (scope.path.clone(), offset));
			write_bytes(staged.path(), &bytes, base).await?;

			if !subject.still_holds(&scope.path).await {
				return Err(stale(
					state::read(context, &scope.root, &scope.path).await?.0,
				));
			}
			still_inside(&scope).await?;
			turn.proceed()?;
			space_events::claim(&[&scope.path]);
			let previous = trash::trash(&scope.path, Some(&context.volume_manager), JobId(id))
				.await
				.map_err(|e| WriteError::Io(format!("set the previous bytes aside: {e}")))?;
			let landed = match commit(&mut staged, scope.path.clone()).await {
				Ok(landed) => landed,
				Err(error) => {
					// Nothing new landed, so the previous bytes go back where they were.
					if let Some(previous) = &previous {
						if let Err(restore) = trash::restore(previous, &scope.path).await {
							warn!(path = %current.path, error = %restore, "could not put back the bytes a failed agent write set aside");
						}
					}
					return Err(error);
				}
			};
			// The mode is applied after the rename, so a read-only original never
			// makes the staged file unwritable before it is flushed.
			let _ = tokio::fs::set_permissions(&landed, metadata.permissions()).await;
			let meta = std::fs::symlink_metadata(&landed).ok();
			if landed == scope.path {
				effects.push(Effect::replaced(landed.clone(), previous, meta.as_ref()));
				(landed, false)
			} else {
				// Something took the name between setting the old bytes aside
				// and the rename; it is kept, and so are both of ours.
				effects.push(Effect::trashed(
					scope.path.clone(),
					previous,
					Some(&metadata),
				));
				effects.push(Effect::created(landed.clone(), meta.as_ref()));
				(landed, true)
			}
		}
	};

	let before = current.content_hash.clone();
	finish(context, &scope, id, effects, &landed, conflict, before).await
}

/// Create a directory and any missing parents.
///
/// A directory already at the path is the answer, unchanged. Anything else
/// at the path is kept, and the directory is made under a conflict name.
pub async fn make_dir(
	context: &CoreContext,
	input: MakeDirInput,
) -> Result<WriteOutcome, WriteError> {
	make_dir_in(context, input, WriteTurn::own().await).await
}

/// [`make_dir`] in a turn the caller already holds.
pub async fn make_dir_in(
	context: &CoreContext,
	input: MakeDirInput,
	turn: WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let scope = writable(context, &input.token, "agent.make_dir", &input.path).await?;
	if let Some(bucket) = Bucket::of(context, &scope).await? {
		let outcome = bucket::make_dir(bucket, &turn).await;
		relist(context, scope.library_id, &scope.path).await;
		return outcome;
	}
	let (current, _) =
		state::read_until(context, &scope.root, &scope.path, turn.cancel.clone()).await?;
	let id = Uuid::now_v7();
	if current.kind == Some(PathKind::Directory) {
		return finish(context, &scope, id, Vec::new(), &scope.path, false, None).await;
	}
	let mut wanted = if current.exists() {
		conflict_name(&scope.path, true)
	} else {
		scope.path.clone()
	};
	turn.proceed()?;
	let mut effects = make_parents(&scope.root, &wanted).await?;
	still_inside(&scope).await?;
	loop {
		space_events::claim(&[&wanted]);
		match tokio::fs::create_dir(&wanted).await {
			Ok(()) => break,
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				wanted = conflict_name(&scope.path, true);
			}
			Err(e) => return Err(WriteError::Io(format!("create directory: {e}"))),
		}
	}
	let meta = std::fs::symlink_metadata(&wanted).ok();
	effects.push(Effect::created(wanted.clone(), meta.as_ref()));
	let conflict = wanted != scope.path;
	finish(context, &scope, id, effects, &wanted, conflict, None).await
}

/// Move or rename a path within the grant, on one volume.
///
/// `expect` is checked against `from`. A taken `to` is kept, and the item
/// lands under a conflict name beside it.
pub async fn move_path(
	context: &CoreContext,
	input: MoveInput,
) -> Result<WriteOutcome, WriteError> {
	move_path_in(context, input, WriteTurn::own().await).await
}

/// [`move_path`] in a turn the caller already holds.
pub async fn move_path_in(
	context: &CoreContext,
	input: MoveInput,
	turn: WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let from = writable(context, &input.token, "agent.move", &input.from).await?;
	let to = writable(context, &input.token, "agent.move", &input.to).await?;
	if let Some(source) = Bucket::of(context, &from).await? {
		let Some(target) = Bucket::of(context, &to).await? else {
			return Err(WriteError::Invalid(
				"agents move within one volume; this move crosses volumes".to_string(),
			));
		};
		let outcome = bucket::move_path(
			context,
			from.library_id,
			from.grant_id,
			source,
			target,
			input.expect,
			&turn,
		)
		.await;
		relist(context, from.library_id, &from.path).await;
		relist(context, to.library_id, &to.path).await;
		return outcome;
	}
	let (current, metadata) =
		state::read_until(context, &from.root, &from.path, turn.cancel.clone()).await?;
	check(&input.expect, &current)?;
	let metadata = metadata.ok_or_else(|| stale(current.clone()))?;
	if to.path.starts_with(&from.path) {
		return Err(WriteError::Invalid(format!(
			"{} cannot move inside itself",
			current.path
		)));
	}
	let is_dir = metadata.is_dir();
	let id = Uuid::now_v7();
	let mut wanted = if std::fs::symlink_metadata(&to.path).is_ok() {
		conflict_name(&to.path, is_dir)
	} else {
		to.path.clone()
	};
	unchanged(context, &from, &current, &metadata, &turn).await?;
	if is_dir {
		no_protected_descendant(context, &from, Some(&wanted), &turn).await?;
	}
	turn.proceed()?;
	let mut effects = make_parents(&to.root, &wanted).await?;
	still_inside(&from).await?;
	still_inside(&to).await?;
	loop {
		let (source, target) = (from.path.clone(), wanted.clone());
		space_events::claim(&[&source, &target]);
		let renamed = tokio::task::spawn_blocking(move || rename_noreplace(&source, &target))
			.await
			.map_err(|e| WriteError::Io(format!("move: {e}")))?;
		match renamed {
			Ok(()) => break,
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				wanted = conflict_name(&to.path, is_dir);
			}
			Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
				return Err(WriteError::Invalid(
					"agents move within one volume; this move crosses volumes".to_string(),
				));
			}
			Err(e) => return Err(WriteError::Io(format!("move: {e}"))),
		}
	}
	let meta = std::fs::symlink_metadata(&wanted).ok();
	effects.push(Effect::moved(
		from.path.clone(),
		wanted.clone(),
		meta.as_ref(),
	));
	let conflict = wanted != to.path;
	let before = current.content_hash.clone();
	let mut outcome = finish(context, &to, id, effects, &wanted, conflict, before).await?;
	outcome.requested = state::shown(&to.root, &to.path);
	Ok(outcome)
}

/// Move a path to the trash.
///
/// The platform's trash where the volume has one, otherwise a Spacedrive
/// trash at the volume's root. Nothing is ever removed for good.
pub async fn delete(context: &CoreContext, input: DeleteInput) -> Result<WriteOutcome, WriteError> {
	delete_in(context, input, WriteTurn::own().await).await
}

/// [`delete`] in a turn the caller already holds.
pub async fn delete_in(
	context: &CoreContext,
	input: DeleteInput,
	turn: WriteTurn,
) -> Result<WriteOutcome, WriteError> {
	let scope = writable(context, &input.token, "agent.delete", &input.path).await?;
	if let Some(target) = Bucket::of(context, &scope).await? {
		let outcome = bucket::delete(
			context,
			scope.library_id,
			scope.grant_id,
			target,
			input.expect,
			&turn,
		)
		.await;
		relist(context, scope.library_id, &scope.path).await;
		return outcome;
	}
	let (current, metadata) =
		state::read_until(context, &scope.root, &scope.path, turn.cancel.clone()).await?;
	check(&input.expect, &current)?;
	let metadata = metadata.ok_or_else(|| stale(current.clone()))?;
	unchanged(context, &scope, &current, &metadata, &turn).await?;
	if metadata.is_dir() {
		no_protected_descendant(context, &scope, None, &turn).await?;
	}
	still_inside(&scope).await?;
	let id = Uuid::now_v7();
	turn.proceed()?;
	space_events::claim(&[&scope.path]);
	let location = trash::trash(&scope.path, Some(&context.volume_manager), JobId(id))
		.await
		.map_err(|e| WriteError::Io(format!("trash: {e}")))?;
	let effects = vec![Effect::trashed(
		scope.path.clone(),
		location,
		Some(&metadata),
	)];
	let before = current.content_hash.clone();
	finish(context, &scope, id, effects, &scope.path, false, before).await
}

async fn writable(
	context: &CoreContext,
	token: &str,
	op: &str,
	path: &Path,
) -> Result<GrantScope, WriteError> {
	let scope = authorize(context, token, op, path, GrantCap::ReadWrite).await?;
	if scope.relative.as_os_str().is_empty() {
		return Err(WriteError::Invalid(
			"name a path inside the granted directory".to_string(),
		));
	}
	Ok(scope)
}

/// Refuse when the path changed since its precondition was checked.
///
/// A file is compared on size and modification time. A directory's own
/// times say nothing about its children, so its listing is hashed again.
async fn unchanged(
	context: &CoreContext,
	scope: &GrantScope,
	checked: &PathState,
	metadata: &std::fs::Metadata,
	turn: &WriteTurn,
) -> Result<(), WriteError> {
	let holds = if metadata.is_dir() {
		let (path, cancel) = (scope.path.clone(), turn.cancel.clone());
		tokio::task::spawn_blocking(move || state::disk(&path, &cancel))
			.await
			.map_err(|e| WriteError::Io(format!("state: {e}")))??
			.content_hash
			== checked.content_hash
	} else {
		Subject::of(metadata).still_holds(&scope.path).await
	};
	if holds {
		return Ok(());
	}
	Err(stale(
		state::read(context, &scope.root, &scope.path).await?.0,
	))
}

/// Refuse to move or delete a directory that holds a path the grant
/// protects, since authorizing the directory does not authorize what is in
/// it. A move also checks where each path would land, under `landing`.
/// Links are not followed: a link is moved or trashed as itself.
async fn no_protected_descendant(
	context: &CoreContext,
	scope: &GrantScope,
	landing: Option<&Path>,
	turn: &WriteTurn,
) -> Result<(), WriteError> {
	let protection = Protection::of(context, &scope.path, scope.library_id).await;
	let landing = match landing {
		Some(landing) => Some(Protection::of(context, landing, scope.library_id).await),
		None => None,
	};
	let (root, dir) = (scope.root.clone(), scope.path.clone());
	let top = dir.clone();
	let cancel = turn.cancel.clone();
	let found = tokio::task::spawn_blocking(move || -> Result<Option<String>, WriteError> {
		let mut pending = vec![dir];
		while let Some(dir) = pending.pop() {
			if cancel.load(Ordering::Relaxed) {
				return Err(WriteError::Cancelled);
			}
			let entries = std::fs::read_dir(&dir)
				.map_err(|e| WriteError::Io(format!("list {}: {e}", state::shown(&root, &dir))))?;
			for entry in entries {
				let entry = entry.map_err(|e| WriteError::Io(format!("list: {e}")))?;
				let path = entry.path();
				let below = path.strip_prefix(&top).unwrap_or(&path);
				let protected = protection.find_below(below).is_some()
					|| landing
						.as_ref()
						.is_some_and(|landing| landing.find_below(below).is_some());
				if protected {
					return Ok(Some(state::shown(&root, &path)));
				}
				if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
					pending.push(path);
				}
			}
		}
		Ok(None)
	})
	.await
	.map_err(|e| WriteError::Io(format!("list: {e}")))??;
	match found {
		Some(path) => Err(GrantError::Protected(path).into()),
		None => Ok(()),
	}
}

/// Resolve the request again just before it lands, so a directory in its
/// path swapped for a link since the grant check is caught rather than
/// written through.
async fn still_inside(scope: &GrantScope) -> Result<(), WriteError> {
	let (root, relative) = (scope.root.clone(), scope.relative.clone());
	let resolved = tokio::task::spawn_blocking(move || scope::resolve(&root, &relative))
		.await
		.map_err(|e| WriteError::Io(format!("resolve: {e}")))??;
	if resolved.path != scope.path {
		return Err(GrantError::Escape(scope.relative.display().to_string()).into());
	}
	Ok(())
}

/// Create the missing directories above `path`, top down, journaling each.
/// Bring a change to a bucket object into its source's records.
async fn relist(context: &CoreContext, library: Uuid, path: &Path) {
	if let Some(library) = context.get_library(library).await {
		crate::ops::sources::rescan::rescan_soon(library, path);
	}
}

async fn make_parents(root: &Path, path: &Path) -> Result<Vec<Effect>, WriteError> {
	let mut missing = Vec::new();
	let mut current = path.parent();
	while let Some(dir) = current {
		if !dir.starts_with(root) {
			break;
		}
		match std::fs::symlink_metadata(dir) {
			Ok(meta) if meta.is_dir() => break,
			Ok(_) => {
				return Err(WriteError::Invalid(format!(
					"{} is not a directory",
					state::shown(root, dir)
				)))
			}
			Err(_) => missing.push(dir.to_path_buf()),
		}
		current = dir.parent();
	}
	let mut effects = Vec::new();
	for dir in missing.into_iter().rev() {
		space_events::claim(&[&dir]);
		match tokio::fs::create_dir(&dir).await {
			Ok(()) => {
				let meta = std::fs::symlink_metadata(&dir).ok();
				effects.push(Effect::created(dir, meta.as_ref()));
			}
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
			Err(e) => return Err(WriteError::Io(format!("create directory: {e}"))),
		}
	}
	Ok(effects)
}

async fn stage(context: &CoreContext, destination: &Path) -> Result<StagedReceive, WriteError> {
	StagedReceive::create(destination, Some(&context.data_dir.join(JOURNAL_DIR)))
		.await
		.map_err(|e| WriteError::Io(format!("stage: {e}")))
}

/// Fill the partial: `bytes` alone, or `bytes` laid over a copy of
/// `base` at an offset.
async fn write_bytes(
	partial: &Path,
	bytes: &[u8],
	base: Option<(PathBuf, u64)>,
) -> Result<(), WriteError> {
	let (partial, bytes) = (partial.to_path_buf(), bytes.to_vec());
	tokio::task::spawn_blocking(move || -> std::io::Result<()> {
		let offset = match base {
			Some((base, offset)) => {
				std::fs::copy(&base, &partial)?;
				offset
			}
			None => 0,
		};
		let mut file = std::fs::OpenOptions::new().write(true).open(&partial)?;
		file.seek(SeekFrom::Start(offset))?;
		file.write_all(&bytes)
	})
	.await
	.map_err(|e| WriteError::Io(format!("write: {e}")))?
	.map_err(|e| WriteError::Io(format!("write: {e}")))
}

/// Rename the partial to `wanted`, or to the next conflict name when
/// something took `wanted` meanwhile. Answers where it landed.
async fn commit(staged: &mut StagedReceive, mut wanted: PathBuf) -> Result<PathBuf, WriteError> {
	loop {
		space_events::claim(&[&wanted]);
		match staged.commit_new(&wanted).await {
			Ok(()) => return Ok(wanted),
			Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
				wanted = conflict_name(&wanted, false);
			}
			Err(e) => return Err(WriteError::Io(format!("commit: {e}"))),
		}
	}
}

/// Journal the effects under the write's id and answer with the result.
async fn finish(
	context: &CoreContext,
	scope: &GrantScope,
	id: Uuid,
	effects: Vec<Effect>,
	landed: &Path,
	conflict: bool,
	before: Option<String>,
) -> Result<WriteOutcome, WriteError> {
	let undo = journal(context, scope.library_id, scope.grant_id, id, &effects).await;
	let (state, _) = state::read(context, &scope.root, landed).await?;
	// Still inside the write's turn, so no other agent write lands between
	// the journal row and this space's record of the change.
	space_events::record_effects(
		context,
		Attribution {
			origin: Origin::AgentWrite,
			actor: scope.actor_id,
			grant: Some(scope.grant_id),
			undo,
			library: Some(scope.library_id),
		},
		&effects,
		Some((landed, before, state.content_hash.clone())),
	)
	.await;
	info!(
		grant = %scope.grant_id,
		path = %state.path,
		conflict,
		effects = effects.len(),
		"agent write"
	);
	Ok(WriteOutcome {
		requested: state::shown(&scope.root, &scope.path),
		state,
		conflict,
		undo,
	})
}

pub(super) async fn journal(
	context: &CoreContext,
	library_id: Uuid,
	grant_id: Uuid,
	id: Uuid,
	effects: &[Effect],
) -> Option<Uuid> {
	if effects.is_empty() {
		return None;
	}
	let library = context.libraries().await.get_library(library_id).await?;
	match library
		.jobs()
		.database()
		.append_journal_for(JobId(id), Some(grant_id), effects)
		.await
	{
		Ok(()) => Some(id),
		Err(e) => {
			// The write landed; only its undo is lost, and the outcome says so.
			warn!(write = %id, error = %e, "could not journal an agent write");
			None
		}
	}
}
