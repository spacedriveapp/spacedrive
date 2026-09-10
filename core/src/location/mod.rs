//! Pinning a place inside a source.
//!
//! A location is a name, a path relative to its source root, and whether
//! someone put it there. It owns no records: the volume index holds those, and
//! deleting a location leaves them alone. What it decides is retention, since a
//! covered subtree is kept whole rather than summarised, and watching.
//!
//! There is no "add a location" gesture any more. The five folders a person
//! recognises are written when the library is created, and beyond that pinning
//! a folder is what makes it a location.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use thiserror::Error;
use uuid::Uuid;

use crate::context::CoreContext;
use crate::domain::Location;
use crate::infra::db::entities::{location, source};
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

/// The default set, written once when a library is created.
///
/// These are the folders a person opening a file manager expects to already be
/// there. A path that does not exist on this machine is not written, so a Linux
/// box without `~/Pictures` gets four rows rather than a broken fifth.
pub fn default_paths() -> Vec<(String, PathBuf)> {
	let Some(home) = dirs::home_dir() else {
		return Vec::new();
	};

	let mut defaults = vec![("Home".to_string(), home.clone())];
	for name in ["Desktop", "Documents", "Downloads", "Pictures"] {
		defaults.push((name.to_string(), home.join(name)));
	}
	defaults.into_iter().filter(|(_, p)| p.exists()).collect()
}

/// Write the defaults for a new library, skipping any path already pinned.
///
/// Failures are reported rather than fatal: a library that opens without
/// Downloads in the sidebar is a worse library, and a library that refuses to
/// open is no library at all.
pub async fn write_defaults(library: &Arc<Library>, context: &Arc<CoreContext>) {
	for (name, path) in default_paths() {
		match pin(
			library,
			context,
			&path,
			name.clone(),
			location::Origin::Default,
		)
		.await
		{
			Ok(_) => tracing::debug!("Pinned {name} at {}", path.display()),
			Err(LocationError::AlreadyPinned(_)) => {}
			Err(error) => tracing::warn!("Could not pin {name}: {error}"),
		}
	}
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
	let rows = location::Entity::find()
		.find_also_related(source::Entity)
		.all(library.db().conn())
		.await?;

	let cache = context.ephemeral_cache();
	let attached: std::collections::HashMap<Uuid, bool> = cache
		.sources()
		.into_iter()
		.map(|status| (status.id, status.attached))
		.collect();

	let mut locations = Vec::with_capacity(rows.len());
	for (model, source) in rows {
		let Some(root) = source.and_then(|source| source.root) else {
			tracing::warn!(location = %model.uuid, "location has no source root, skipping");
			continue;
		};

		let is_available = attached.get(&model.source_uuid).copied().unwrap_or(false);
		let Some(mut location) = Location::from_row(model, root, is_available, None) else {
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

/// The paths on a volume that are kept at full fidelity.
///
/// This is what `Retention.covered` wants: everything else on the drive is
/// mapped for structure and counted below [`SUMMARY_DEPTH`]. Overlapping pins
/// are a union rather than a contest, so a location inside a location costs
/// nothing.
///
/// [`SUMMARY_DEPTH`]: crate::ops::indexing::summary::SUMMARY_DEPTH
pub async fn covered_paths(
	library: &Arc<Library>,
	context: &Arc<CoreContext>,
	mount_point: &Path,
) -> Vec<PathBuf> {
	match list(library, context).await {
		Ok(locations) => locations
			.into_iter()
			.filter_map(|location| location.path().map(Path::to_path_buf))
			.filter(|path| path.starts_with(mount_point))
			.collect(),
		Err(error) => {
			tracing::warn!(
				"Could not read locations for {}: {error}",
				mount_point.display()
			);
			Vec::new()
		}
	}
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
