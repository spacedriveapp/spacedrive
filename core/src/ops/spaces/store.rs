//! Finding the store a space lives in.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use chrono::Utc;
use tracing::warn;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::{Space, SpaceKind},
	infra::{action::error::ActionError, job::types::JobPriority},
	library::Library,
	ops::{
		indexing::{
			content_identity::ContentIdentityJob, content_text::ContentTextJob, deep_scope,
			store::SourceStore,
		},
		sources::registry,
	},
};

/// Held while a space is created, so two requests for one folder cannot both
/// pass the check for an existing space before either writes.
pub(super) static CREATES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A folder resolved to the source of this library that holds it.
pub struct Located {
	pub source_id: Uuid,
	pub root: PathBuf,
	/// The folder relative to the source root, `/`-separated.
	pub relative: String,
	pub store: Arc<SourceStore>,
}

/// The source of `library` that holds `path`, with its store open for writes.
///
/// A source is rooted at its volume's spelling of a path, which on macOS is
/// not the one a person types: /Users/me lives at /System/Volumes/Data/Users/me.
pub async fn locate(library: &Library, context: &CoreContext, path: &Path) -> Option<Located> {
	let index = context.volume_index();
	let spelled = context
		.volume_manager
		.locate_path(path)
		.await
		.map(|(_, spelled)| spelled);
	let candidates: Vec<&Path> = spelled
		.iter()
		.map(PathBuf::as_path)
		.chain(std::iter::once(path))
		.collect();
	let source_id = candidates
		.iter()
		.find_map(|candidate| index.source_id_for(candidate))?;
	// The index resolves a path whatever library asks; only this library's
	// sources hold its spaces.
	registry::get(library.db().conn(), source_id).await.ok()?;
	let root = index.source_root(source_id)?;
	let relative = candidates
		.iter()
		.find_map(|candidate| candidate.strip_prefix(&root).ok())
		.map(slash_path)?;
	let store = index.store_for(&root).await?;
	Some(Located {
		source_id,
		root,
		relative,
		store,
	})
}

/// Every live space in the sources of `library` whose stores this device has.
pub async fn all(library: &Library, context: &CoreContext) -> Result<Vec<Space>, String> {
	let index = context.volume_index();
	let mut spaces = Vec::new();
	for source in index.sources_of(library.id()) {
		if source.attached {
			if let Some(store) = index.store_for(&source.root).await {
				if let Err(error) = adopt_deep_scope(context, source.id, &source.root, &store).await
				{
					warn!(source = %source.id, %error, "could not turn a source's deep scope into spaces");
				}
			}
		}
		let Some(db) = index.read_store(source.id).await else {
			continue;
		};
		let root = source.attached.then_some(source.root.as_path());
		for row in db.spaces().await.map_err(|e| e.to_string())? {
			spaces.push(Space::from_row(row, source.id, root));
		}
	}
	spaces.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	Ok(spaces)
}

/// Make the folders a source reads by default spaces, once.
///
/// The home folder reads its standard user folders until someone says
/// otherwise, and a list set before spaces existed is kept in the store's
/// `deep_scope` assertion. Each of those folders becomes a space with
/// content indexing on, so what is read is one list a person can see and
/// change. The assertion is then set to an empty list, which marks the
/// source as adopted and stops the default from coming back after someone
/// deletes one of the spaces. The ids derive from the source and the
/// folder, so two devices that adopt the same source write the same spaces.
pub async fn adopt_deep_scope(
	context: &CoreContext,
	source_id: Uuid,
	root: &Path,
	store: &SourceStore,
) -> Result<(), String> {
	let db = store.db();
	let _one_create = CREATES.lock().await;
	let kept = match db.deep_scope().await.map_err(|e| e.to_string())? {
		Some(kept) => kept,
		None => {
			let default = deep_scope::default_scope(root)
				.await
				.map_err(|e| e.to_string())?;
			// A source that reads its whole root by default needs no space
			// to say so.
			if default == [String::new()] {
				return Ok(());
			}
			default
		}
	};
	if kept.is_empty() {
		return Ok(());
	}
	let existing = db.spaces().await.map_err(|e| e.to_string())?;
	let stamp = stamp(context)?;
	for subtree in kept {
		if existing.iter().any(|space| space.root == subtree) {
			continue;
		}
		let name = subtree
			.rsplit('/')
			.next()
			.filter(|name| !name.is_empty())
			.map(str::to_string)
			.or_else(|| {
				root.file_name()
					.map(|name| name.to_string_lossy().into_owned())
			})
			.unwrap_or_else(|| "Home".to_string());
		db.put_space(&sd_store::SpaceRow {
			uuid: Uuid::new_v5(&source_id, subtree.as_bytes()),
			name,
			kind: SpaceKind::Folder.as_str().to_string(),
			root: subtree,
			content_indexing: true,
			created_at: Utc::now().to_rfc3339(),
			deleted: false,
			updated_hlc: stamp.hlc.clone(),
			origin_device: stamp.device_uuid,
		})
		.await
		.map_err(|e| e.to_string())?;
	}
	db.set_deep_scope(&[], &stamp)
		.await
		.map_err(|e| e.to_string())?;
	Ok(())
}

/// Start the pass that follows a change to what a source reads: reading a
/// newly covered folder, or dropping the text of one no longer covered.
/// Hashes already taken stay, because duplicate and integrity answers stand
/// on them. Answers the job, or None when the source does not read contents.
pub async fn reread(
	library: &Library,
	context: &CoreContext,
	source_id: Uuid,
	reading: bool,
) -> Result<Option<Uuid>, ActionError> {
	let index = context.volume_index();
	let Some(root) = index.source_root(source_id) else {
		return Ok(None);
	};
	let reads_contents = index
		.source_config(source_id)
		.is_some_and(|config| config.identify_content);
	if !reads_contents || index.dispatch_refusal(&root).is_some() {
		return Ok(None);
	}
	let handle = if reading {
		// Identification queues the text and image passes behind it.
		library
			.jobs()
			.dispatch_with_priority(ContentIdentityJob::new(root), JobPriority::LOW, None)
			.await?
			.id()
	} else {
		library
			.jobs()
			.dispatch_with_priority(ContentTextJob::new(root, false), JobPriority::LOW, None)
			.await?
			.id()
	};
	Ok(Some(handle.0))
}

/// One live space by id.
pub async fn find(library: &Library, context: &CoreContext, id: Uuid) -> Result<Space, String> {
	all(library, context)
		.await?
		.into_iter()
		.find(|space| space.id == id)
		.ok_or_else(|| format!("no space {id} in this library"))
}

/// The writable store of the source holding `space`.
pub async fn writable(context: &CoreContext, space: &Space) -> Result<Arc<SourceStore>, String> {
	let index = context.volume_index();
	let root = index
		.source_root(space.source_id)
		.ok_or_else(|| "the source holding this space is not on this device".to_string())?;
	index
		.store_for(&root)
		.await
		.ok_or_else(|| "the source holding this space has no store on this device".to_string())
}

/// A relative path in the store's spelling: `/`-separated on every platform.
pub fn slash_path(path: &Path) -> String {
	path.components()
		.map(|component| component.as_os_str().to_string_lossy())
		.collect::<Vec<_>>()
		.join("/")
}

/// The stamp for a space written on this device.
pub fn stamp(context: &CoreContext) -> Result<sd_store::Stamp, String> {
	let device = context
		.device_manager
		.device_id()
		.map_err(|e| format!("no device identity: {e}"))?;
	Ok(crate::ops::tags::stamp::assertion_stamp(device))
}
