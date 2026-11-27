//! Finding the store a space lives in.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::Space,
	library::Library,
	ops::{indexing::store::SourceStore, sources::registry},
};

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
