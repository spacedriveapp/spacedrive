//! The four writes and the stat that feeds their preconditions.
//!
//! Every write takes one process-wide lock from its grant check to its
//! journal row, so two agents on this daemon cannot interleave a check with
//! a commit. Writers outside Spacedrive are not stopped by it; they are
//! caught by checking the target again just before the rename.

use std::{
	io::{Seek, SeekFrom, Write},
	path::{Path, PathBuf},
};

use base64::Engine;
use tokio::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use super::{
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
		grants::{authorize, scope, GrantCap, GrantError, GrantScope},
	},
	service::network::protocol::staged_receive::{rename_noreplace, StagedReceive, JOURNAL_DIR},
};

static WRITES: Mutex<()> = Mutex::const_new(());

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

	let _one_writer = WRITES.lock().await;
	let scope = writable(context, &input.token, "agent.write_file", &input.path).await?;
	let (current, metadata) = state::read(context, &scope.root, &scope.path).await?;
	let id = Uuid::now_v7();
	let mut effects = Vec::new();

	let (landed, conflict) = match &input.expect {
		None => {
			let wanted = if current.exists() {
				conflict_name(&scope.path, false)
			} else {
				scope.path.clone()
			};
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

	finish(context, &scope, id, effects, &landed, conflict).await
}

/// Create a directory and any missing parents.
///
/// A directory already at the path is the answer, unchanged. Anything else
/// at the path is kept, and the directory is made under a conflict name.
pub async fn make_dir(
	context: &CoreContext,
	input: MakeDirInput,
) -> Result<WriteOutcome, WriteError> {
	let _one_writer = WRITES.lock().await;
	let scope = writable(context, &input.token, "agent.make_dir", &input.path).await?;
	let (current, _) = state::read(context, &scope.root, &scope.path).await?;
	let id = Uuid::now_v7();
	if current.kind == Some(PathKind::Directory) {
		return finish(context, &scope, id, Vec::new(), &scope.path, false).await;
	}
	let mut wanted = if current.exists() {
		conflict_name(&scope.path, true)
	} else {
		scope.path.clone()
	};
	let mut effects = make_parents(&scope.root, &wanted).await?;
	still_inside(&scope).await?;
	loop {
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
	finish(context, &scope, id, effects, &wanted, conflict).await
}

/// Move or rename a path within the grant, on one volume.
///
/// `expect` is checked against `from`. A taken `to` is kept, and the item
/// lands under a conflict name beside it.
pub async fn move_path(
	context: &CoreContext,
	input: MoveInput,
) -> Result<WriteOutcome, WriteError> {
	let _one_writer = WRITES.lock().await;
	let from = writable(context, &input.token, "agent.move", &input.from).await?;
	let to = writable(context, &input.token, "agent.move", &input.to).await?;
	let (current, metadata) = state::read(context, &from.root, &from.path).await?;
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
	let mut effects = make_parents(&to.root, &wanted).await?;

	unchanged(context, &from, &current, &metadata).await?;
	if is_dir {
		no_protected_descendant(&from).await?;
	}
	still_inside(&from).await?;
	still_inside(&to).await?;
	loop {
		let (source, target) = (from.path.clone(), wanted.clone());
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
	let mut outcome = finish(context, &to, id, effects, &wanted, conflict).await?;
	outcome.requested = state::shown(&to.root, &to.path);
	Ok(outcome)
}

/// Move a path to the trash.
///
/// The platform's trash where the volume has one, otherwise a Spacedrive
/// trash at the volume's root. Nothing is ever removed for good.
pub async fn delete(context: &CoreContext, input: DeleteInput) -> Result<WriteOutcome, WriteError> {
	let _one_writer = WRITES.lock().await;
	let scope = writable(context, &input.token, "agent.delete", &input.path).await?;
	let (current, metadata) = state::read(context, &scope.root, &scope.path).await?;
	check(&input.expect, &current)?;
	let metadata = metadata.ok_or_else(|| stale(current.clone()))?;
	unchanged(context, &scope, &current, &metadata).await?;
	if metadata.is_dir() {
		no_protected_descendant(&scope).await?;
	}
	still_inside(&scope).await?;
	let id = Uuid::now_v7();
	let location = trash::trash(&scope.path, Some(&context.volume_manager), JobId(id))
		.await
		.map_err(|e| WriteError::Io(format!("trash: {e}")))?;
	let effects = vec![Effect::trashed(
		scope.path.clone(),
		location,
		Some(&metadata),
	)];
	finish(context, &scope, id, effects, &scope.path, false).await
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
) -> Result<(), WriteError> {
	let holds = if metadata.is_dir() {
		let path = scope.path.clone();
		tokio::task::spawn_blocking(move || state::disk(&path))
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
/// it. Links are not followed: a link is moved or trashed as itself.
async fn no_protected_descendant(scope: &GrantScope) -> Result<(), WriteError> {
	let (root, dir) = (scope.root.clone(), scope.path.clone());
	let found = tokio::task::spawn_blocking(move || -> Result<Option<String>, WriteError> {
		let mut pending = vec![dir];
		while let Some(dir) = pending.pop() {
			let entries = std::fs::read_dir(&dir)
				.map_err(|e| WriteError::Io(format!("list {}: {e}", state::shown(&root, &dir))))?;
			for entry in entries {
				let entry = entry.map_err(|e| WriteError::Io(format!("list: {e}")))?;
				let path = entry.path();
				if scope::protected_component(Path::new(&entry.file_name())).is_some() {
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
) -> Result<WriteOutcome, WriteError> {
	let undo = journal(context, scope.library_id, id, &effects).await;
	let (state, _) = state::read(context, &scope.root, landed).await?;
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

async fn journal(
	context: &CoreContext,
	library_id: Uuid,
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
		.append_journal(JobId(id), effects)
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
