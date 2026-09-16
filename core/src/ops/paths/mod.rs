//! # Path Context
//!
//! A path bar needs to explain what Spacedrive knows about the path without
//! turning locations back into indexing switches. This query joins the live
//! volume map, source store, watcher, and navigation pins into one typed read.

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	context::CoreContext,
	domain::addressing::SdPath,
	infra::{
		db::entities::location,
		query::{LibraryQuery, QueryError, QueryResult},
	},
	ops::indexing::ephemeral::cache::SourceStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathContextInput {
	pub path: SdPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PathAvailability {
	Available,
	PermissionDenied,
	Missing,
	Unavailable,
	Remote,
	Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PathMapState {
	Unseen,
	Indexing,
	Detailed,
	Summarised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PathWatcherState {
	Active,
	Inactive,
	Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathVolumeContext {
	pub id: Uuid,
	pub name: String,
	pub mount_point: PathBuf,
	pub tracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathSourceContext {
	pub id: Uuid,
	pub name: String,
	pub root: PathBuf,
	pub attached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathLocationContext {
	pub id: Uuid,
	pub name: String,
	pub root: SdPath,
	pub origin: location::Origin,
	pub exact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathStorageContext {
	/// The active volume arena can answer reads about this path.
	pub memory: bool,
	/// The volume arena has a machine-local restart snapshot.
	pub restart_cache: bool,
	/// The source has created its durable record store.
	pub source_store: bool,
	/// The current path is covered by committed records in that source store.
	pub source_record: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathContextOutput {
	/// The path as the owning volume spells it. This removes APFS aliases from
	/// all containment checks while the explorer can keep displaying its input.
	pub canonical_path: SdPath,
	pub availability: PathAvailability,
	pub map_state: PathMapState,
	pub indexing_root: Option<PathBuf>,
	pub watcher_state: PathWatcherState,
	pub watcher_root: Option<PathBuf>,
	pub volume: Option<PathVolumeContext>,
	pub source: Option<PathSourceContext>,
	/// The closest explicit pin containing this path, if one exists.
	pub location: Option<PathLocationContext>,
	/// A computed system Place at this exact path, such as Desktop.
	pub system_place: Option<String>,
	pub storage: PathStorageContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PathContextQuery {
	input: PathContextInput,
}

impl LibraryQuery for PathContextQuery {
	type Input = PathContextInput;
	type Output = PathContextOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let Some(input_path) = self.input.path.as_local_path().map(Path::to_path_buf) else {
			let availability = if matches!(self.input.path, SdPath::Physical { .. }) {
				PathAvailability::Remote
			} else {
				PathAvailability::Unsupported
			};
			return Ok(empty_context(self.input.path, availability));
		};

		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;
		let library = context
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;

		let located_volume = context.volume_manager.locate_path(&input_path).await;
		let canonical = located_volume
			.as_ref()
			.map(|(_, path)| path.clone())
			.unwrap_or_else(|| input_path.canonicalize().unwrap_or(input_path));
		let canonical_path = SdPath::local(canonical.clone());

		let availability = availability(&canonical).await;
		let cache = context.ephemeral_cache();
		let source_status = longest_source(cache.sources(), &canonical);
		let source = source_status.as_ref().map(|status| PathSourceContext {
			id: status.id,
			name: cache
				.source_name(status.id)
				.unwrap_or_else(|| display_name(&status.root)),
			root: status.root.clone(),
			attached: status.attached,
		});

		let indexing_root = longest_ancestor(cache.paths_in_progress(), &canonical);
		let indexed_root = longest_ancestor(cache.indexed_paths(), &canonical);
		let summarised = if indexed_root.is_some() {
			let index = cache.resolve_index(&canonical);
			let index = index.read().await;
			longest_ancestor(index.summarised_paths(), &canonical).is_some()
		} else {
			false
		};
		let map_state = if indexing_root.is_some() {
			PathMapState::Indexing
		} else if summarised {
			PathMapState::Summarised
		} else if indexed_root.is_some() {
			PathMapState::Detailed
		} else {
			PathMapState::Unseen
		};

		let watcher_root = cache.find_watched_root(&canonical);
		let watcher_state = match availability {
			PathAvailability::Available => {
				if watcher_root.is_some() {
					PathWatcherState::Active
				} else {
					PathWatcherState::Inactive
				}
			}
			_ => PathWatcherState::Unavailable,
		};

		let restart_cache = match cache.snapshot_path_for(&canonical) {
			Some(path) => tokio::fs::try_exists(path).await.unwrap_or(false),
			None => false,
		};
		let source_store = match source_status
			.as_ref()
			.and_then(|status| status.directory.as_ref())
		{
			Some(directory) => tokio::fs::try_exists(directory.join("data.db"))
				.await
				.unwrap_or(false),
			None => false,
		};
		let source_record = if source_store {
			match cache.store_for(&canonical).await {
				Some(store) => store.contains_path(&canonical).await,
				None => false,
			}
		} else {
			false
		};

		let location = match source_status.as_ref() {
			Some(status) => {
				let rows = location::Entity::find()
					.filter(location::Column::SourceUuid.eq(status.id))
					.all(library.db().conn())
					.await?;
				closest_location(rows, &status.root, &canonical)
			}
			None => None,
		};
		let system_place = system_place_for(&context, &canonical).await;

		Ok(PathContextOutput {
			canonical_path,
			availability,
			map_state,
			indexing_root,
			watcher_state,
			watcher_root,
			volume: located_volume.map(|(volume, _)| PathVolumeContext {
				id: volume.id,
				name: volume.name,
				mount_point: volume.mount_point,
				tracked: volume.is_tracked,
			}),
			source,
			location,
			system_place,
			storage: PathStorageContext {
				memory: map_state != PathMapState::Unseen,
				restart_cache,
				source_store,
				source_record,
			},
		})
	}
}

async fn availability(path: &Path) -> PathAvailability {
	match tokio::fs::metadata(path).await {
		Ok(_) => PathAvailability::Available,
		Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
			PathAvailability::PermissionDenied
		}
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => PathAvailability::Missing,
		Err(_) => PathAvailability::Unavailable,
	}
}

fn empty_context(path: SdPath, availability: PathAvailability) -> PathContextOutput {
	PathContextOutput {
		canonical_path: path,
		availability,
		map_state: PathMapState::Unseen,
		indexing_root: None,
		watcher_state: PathWatcherState::Unavailable,
		watcher_root: None,
		volume: None,
		source: None,
		location: None,
		system_place: None,
		storage: PathStorageContext {
			memory: false,
			restart_cache: false,
			source_store: false,
			source_record: false,
		},
	}
}

fn longest_source(sources: Vec<SourceStatus>, path: &Path) -> Option<SourceStatus> {
	sources
		.into_iter()
		.filter(|source| !source.root.as_os_str().is_empty() && path.starts_with(&source.root))
		.max_by_key(|source| source.root.as_os_str().len())
}

fn longest_ancestor(paths: Vec<PathBuf>, path: &Path) -> Option<PathBuf> {
	paths
		.into_iter()
		.filter(|root| path.starts_with(root))
		.max_by_key(|root| root.as_os_str().len())
}

fn closest_location(
	rows: Vec<location::Model>,
	source_root: &Path,
	path: &Path,
) -> Option<PathLocationContext> {
	rows.into_iter()
		.filter_map(|row| {
			if !row.relative_path.is_empty()
				&& Path::new(&row.relative_path)
					.components()
					.any(|component| !matches!(component, std::path::Component::Normal(_)))
			{
				return None;
			}
			let root = if row.relative_path.is_empty() {
				source_root.to_path_buf()
			} else {
				source_root.join(&row.relative_path)
			};
			path.starts_with(&root).then_some((row, root))
		})
		.max_by_key(|(_, root)| root.as_os_str().len())
		.map(|(row, root)| PathLocationContext {
			id: row.uuid,
			name: row.name,
			origin: location::Origin::from(row.origin.as_str()),
			exact: path == root,
			root: SdPath::local(root),
		})
}

async fn system_place_for(context: &Arc<CoreContext>, path: &Path) -> Option<String> {
	for (name, known_path) in crate::location::known_paths() {
		let canonical = context
			.volume_manager
			.locate_path(&known_path)
			.await
			.map(|(_, path)| path)
			.unwrap_or_else(|| known_path.canonicalize().unwrap_or(known_path));
		if path == canonical {
			return Some(name);
		}
	}
	None
}

fn display_name(path: &Path) -> String {
	path.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| path.to_string_lossy().into_owned())
}

crate::register_library_query!(PathContextQuery, "paths.context");

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Utc;

	fn location(relative_path: &str, name: &str) -> location::Model {
		location::Model {
			id: 1,
			uuid: Uuid::now_v7(),
			source_uuid: Uuid::now_v7(),
			relative_path: relative_path.to_string(),
			name: name.to_string(),
			origin: "user".to_string(),
			created_at: Utc::now(),
		}
	}

	fn source(root: &str) -> SourceStatus {
		SourceStatus {
			id: Uuid::now_v7(),
			root: PathBuf::from(root),
			volume_uuid: None,
			attached: true,
			restored: false,
			last_seen_secs: 0,
			entry_count: None,
			total_bytes: None,
			directory: None,
			thumbs_path: None,
		}
	}

	#[test]
	fn path_components_choose_the_nearest_ancestor() {
		let path = Path::new("/data/projects/spacedrive/docs");
		assert_eq!(
			longest_ancestor(
				vec![PathBuf::from("/data"), PathBuf::from("/data/projects")],
				path,
			),
			Some(PathBuf::from("/data/projects"))
		);
		assert!(longest_ancestor(vec![PathBuf::from("/database")], path).is_none());
	}

	#[test]
	fn nested_source_owns_the_path() {
		let result = longest_source(
			vec![source("/data"), source("/data/projects")],
			Path::new("/data/projects/spacedrive"),
		)
		.expect("matching source");

		assert_eq!(result.root, Path::new("/data/projects"));
	}

	#[test]
	fn closest_pin_is_source_relative_and_component_safe() {
		let root = Path::new("/data");
		let result = closest_location(
			vec![location("projects", "Projects"), location("pro", "Pro")],
			root,
			Path::new("/data/projects/spacedrive"),
		)
		.expect("matching pin");

		assert_eq!(result.name, "Projects");
		assert!(!result.exact);
	}

	#[test]
	fn parent_components_cannot_escape_a_source() {
		assert!(closest_location(
			vec![location("../private", "Bad")],
			Path::new("/data"),
			Path::new("/private"),
		)
		.is_none());
		assert!(closest_location(
			vec![location("/private", "Also bad")],
			Path::new("/data"),
			Path::new("/private"),
		)
		.is_none());
		assert!(crate::domain::Location::from_row(
			location("/private", "Also bad"),
			"/data".to_string(),
			true,
			None,
		)
		.is_none());
	}
}
