//! Pinning a place inside a source.
//!
//! A location is a name, a path relative to its source root, and whether
//! someone put it there. It owns no records: the volume index holds those, and
//! deleting a location leaves them alone. It does not change indexing or
//! watching.
//!
//! Known folders such as Desktop are computed per device and presented as
//! Places. Pinning one is the explicit act that makes it a location.

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use thiserror::Error;
use uuid::Uuid;

use crate::context::CoreContext;
use crate::domain::Location;
use crate::infra::db::entities::location;
use crate::library::Library;

#[derive(Debug, Error)]
pub enum LocationError {
	#[error("Database error: {0}")]
	Database(#[from] sea_orm::DbErr),
	#[error("Path does not exist: {}", .0.display())]
	PathNotFound(PathBuf),
	#[error("{} is not inside any source", .0.display())]
	NoSource(PathBuf),
	#[error("Already pinned: {}", .0.display())]
	AlreadyPinned(PathBuf),
	#[error("Location not found: {0}")]
	NotFound(Uuid),
}

pub type LocationResult<T> = Result<T, LocationError>;

/// Known folders this device can offer as navigation destinations.
///
/// These are the folders a person opening a file manager expects to already be
/// there. A path that does not exist on this machine is not written, so a Linux
/// box without `~/Pictures` gets four rows rather than a broken fifth.
pub fn known_paths() -> Vec<(String, PathBuf)> {
	let Some(home) = dirs::home_dir() else {
		return Vec::new();
	};

	let candidates = [
		("Home", Some(home)),
		("Desktop", dirs::desktop_dir()),
		("Documents", dirs::document_dir()),
		("Downloads", dirs::download_dir()),
		("Pictures", dirs::picture_dir()),
		("Movies", dirs::video_dir()),
		("Music", dirs::audio_dir()),
	];
	let mut seen = HashSet::new();
	candidates
		.into_iter()
		.filter_map(|(name, path)| path.map(|path| (name.to_string(), path)))
		.filter(|(_, path)| path.exists() && seen.insert(path.clone()))
		.collect()
}

/// Pin a path, making it a location.
///
/// The source is the innermost one containing the path, so a folder inside a
/// nested source belongs to that source rather than to the drive above it.
pub async fn pin(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	path: &Path,
	name: String,
	origin: location::Origin,
) -> LocationResult<Location> {
	if !path.exists() {
		return Err(LocationError::PathNotFound(path.to_path_buf()));
	}

	let (source_id, source_root) = innermost_source(context, path)
		.ok_or_else(|| LocationError::NoSource(path.to_path_buf()))?;

	let relative = relative_to(&source_root, path);
	let db = library.db().conn();

	let existing = location::Entity::find()
		.filter(location::Column::SourceUuid.eq(source_id))
		.filter(location::Column::RelativePath.eq(relative.clone()))
		.one(db)
		.await?;
	if existing.is_some() {
		return Err(LocationError::AlreadyPinned(path.to_path_buf()));
	}

	let model = location::ActiveModel {
		uuid: Set(Uuid::now_v7()),
		source_uuid: Set(source_id),
		relative_path: Set(relative),
		name: Set(name),
		origin: Set(origin.as_str().to_string()),
		created_at: Set(chrono::Utc::now()),
		..Default::default()
	}
	.insert(db)
	.await?;

	let root = source_root.to_string_lossy().to_string();
	Location::from_row(model, root, true, None)
		.ok_or_else(|| LocationError::NoSource(path.to_path_buf()))
}

/// Unpin, which deletes the row and nothing else.
pub async fn unpin(library: &Arc<Library>, id: Uuid) -> LocationResult<()> {
	let deleted = location::Entity::delete_many()
		.filter(location::Column::Uuid.eq(id))
		.exec(library.db().conn())
		.await?;

	if deleted.rows_affected == 0 {
		return Err(LocationError::NotFound(id));
	}
	Ok(())
}

/// Rename a location. The path is not editable: a pin somewhere else is a
/// different pin.
pub async fn rename(library: &Arc<Library>, id: Uuid, name: String) -> LocationResult<()> {
	let updated = location::Entity::update_many()
		.filter(location::Column::Uuid.eq(id))
		.set(location::ActiveModel {
			name: Set(name),
			..Default::default()
		})
		.exec(library.db().conn())
		.await?;

	if updated.rows_affected == 0 {
		return Err(LocationError::NotFound(id));
	}
	Ok(())
}

/// Every location in the library, with its size and file count read off the
/// volume index rather than off a stored total.
pub async fn list(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
) -> LocationResult<Vec<Location>> {
	let rows = location::Entity::find().all(library.db().conn()).await?;

	let cache = context.ephemeral_cache();
	let sources: std::collections::HashMap<Uuid, _> = cache
		.sources()
		.into_iter()
		.map(|status| (status.id, status))
		.collect();

	let mut locations = Vec::with_capacity(rows.len());
	for model in rows {
		let Some(source) = sources.get(&model.source_uuid) else {
			tracing::warn!(location = %model.uuid, "location source is not registered, skipping");
			continue;
		};

		// The source table stores an anchor relative to its volume. The registry
		// resolves that anchor to the current mount, so pins follow remounts.
		let root = source.root.to_string_lossy().into_owned();
		let Some(mut location) = Location::from_row(model, root, source.attached, None) else {
			continue;
		};

		if let Some(path) = location.path().map(Path::to_path_buf) {
			if let Some(index) = cache.get_for_search(&path) {
				let index = index.read().await;
				location.total_size = index.subtree_size(&path);
				location.file_count = index.subtree_file_count(&path);
			}
		}

		locations.push(location);
	}

	Ok(locations)
}

/// The source containing this path, innermost first.
///
/// Sources nest, so the longest matching root wins: a path under both
/// `/Volumes/Work` and `/Volumes/Work/Media` belongs to the latter, and its
/// records are in that store.
fn innermost_source(context: &Arc<CoreContext>, path: &Path) -> Option<(Uuid, PathBuf)> {
	context
		.ephemeral_cache()
		.sources()
		.into_iter()
		.filter(|status| path.starts_with(&status.root))
		.max_by_key(|status| status.root.as_os_str().len())
		.map(|status| (status.id, status.root))
}

/// A path relative to its source root. Empty for the root itself.
fn relative_to(root: &Path, path: &Path) -> String {
	path.strip_prefix(root)
		.map(|rest| rest.to_string_lossy().to_string())
		.unwrap_or_default()
}
