//! A place someone cares about.
//!
//! Locations used to be how Spacedrive found out a file existed: nothing walked
//! a drive, so a path nobody had named was a path nobody had seen. The storage
//! map removed that premise. A location now preserves a named, source-relative
//! navigation target. It does not change capture, retention, or watcher state.
//!
//! It owns no records. Deleting one is a row going away, and the map does not
//! notice, which is what keeps the concept from growing a second index behind it.

use crate::domain::addressing::SdPath;
use crate::domain::resource::Identifiable;
use crate::infra::db::entities::location::Origin;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A pinned subtree of a source.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Location {
	pub id: Uuid,

	/// The source holding this subtree's records.
	pub source_id: Uuid,

	/// Absolute, rebuilt from the source root and the stored relative path, so
	/// it follows a drive that mounts somewhere else.
	pub sd_path: SdPath,

	pub name: String,

	/// Whether someone pinned this or it came with the library.
	pub origin: Origin,

	/// Rolled up by the volume index rather than stored, so they are current
	/// rather than as of the last scan. `None` while the drive is detached and
	/// the index has not been restored.
	pub total_size: Option<u64>,
	pub file_count: Option<u32>,

	/// Whether the drive holding it is here right now.
	pub is_available: bool,

	pub created_at: DateTime<Utc>,
}

impl Identifiable for Location {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"location"
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::infra::db::entities::{location, source};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let rows = location::Entity::find()
			.filter(location::Column::Uuid.is_in(ids.to_vec()))
			.find_also_related(source::Entity)
			.all(db)
			.await?;

		// Rollups need the arena, which an event emission does not have, so
		// these carry structure only. `locations.list` is where the numbers come
		// from.
		Ok(rows
			.into_iter()
			.filter_map(|(location, source)| Self::from_row(location, source?.root?, false, None))
			.collect())
	}
}

crate::register_resource!(Location);

impl Location {
	/// Assemble a location from its row and the root of the source holding it.
	///
	/// `None` when the stored path escapes the source root, which is the shape
	/// a hand-edited row leaves behind.
	pub fn from_row(
		model: crate::infra::db::entities::location::Model,
		source_root: String,
		is_available: bool,
		rollup: Option<(u64, u32)>,
	) -> Option<Self> {
		if !model.relative_path.is_empty()
			&& std::path::Path::new(&model.relative_path)
				.components()
				.any(|component| !matches!(component, std::path::Component::Normal(_)))
		{
			tracing::warn!(location = %model.uuid, "location path escapes its source, skipping");
			return None;
		}

		let mut path = std::path::PathBuf::from(source_root);
		if !model.relative_path.is_empty() {
			path.push(&model.relative_path);
		}

		Some(Self {
			id: model.uuid,
			source_id: model.source_uuid,
			sd_path: SdPath::local(path),
			name: model.name,
			origin: Origin::from(model.origin.as_str()),
			total_size: rollup.map(|(bytes, _)| bytes),
			file_count: rollup.map(|(_, files)| files),
			is_available,
			created_at: model.created_at,
		})
	}

	/// The local path this location covers, if it has one.
	pub fn path(&self) -> Option<&std::path::Path> {
		self.sd_path.as_local_path()
	}
}
