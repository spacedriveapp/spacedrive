//! # Path Context
//!
//! A path bar needs to explain what Spacedrive knows about a path. This query
//! joins the live volume map, source store, and watcher into one typed read.
//! `paths.system_folders` answers the other question clients ask about paths:
//! which folders a person expects a file manager to already know.

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
	infra::query::{LibraryQuery, QueryError, QueryResult},
	ops::indexing::volume_index::SourceStatus,
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
		let cache = context.volume_index();
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

/// The system folders a person expects a file manager to already know:
/// home, desktop, documents, downloads, pictures, movies, music. A path that
/// does not exist on this machine is not offered.
fn known_paths() -> Vec<(String, std::path::PathBuf)> {
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
	let mut seen = std::collections::HashSet::new();
	candidates
		.into_iter()
		.filter_map(|(name, path)| path.map(|path| (name.to_string(), path)))
		.filter(|(_, path)| path.exists() && seen.insert(path.clone()))
		.collect()
}

async fn system_place_for(context: &Arc<CoreContext>, path: &Path) -> Option<String> {
	for (name, known_path) in known_paths() {
		if path == canonical_spelling(context, &known_path).await {
			return Some(name);
		}
	}
	None
}

/// A path as its owning volume spells it. On macOS this keeps Home from
/// existing twice, once under `/Users` and once under `/System/Volumes/Data`.
async fn canonical_spelling(context: &Arc<CoreContext>, path: &Path) -> PathBuf {
	context
		.volume_manager
		.locate_path(path)
		.await
		.map(|(_, spelled)| spelled)
		.unwrap_or_else(|| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SystemFoldersInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SystemFolder {
	pub name: String,
	/// As the operating system names it, for display.
	pub path: PathBuf,
	/// The volume's spelling on this device, for navigation and containment.
	pub sd_path: SdPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SystemFoldersOutput {
	pub folders: Vec<SystemFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemFoldersQuery;

impl LibraryQuery for SystemFoldersQuery {
	type Input = SystemFoldersInput;
	type Output = SystemFoldersOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self)
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		_session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let device_slug = crate::device::get_current_device_slug();
		let mut folders = Vec::new();
		for (name, path) in known_paths() {
			let spelled = canonical_spelling(&context, &path).await;
			folders.push(SystemFolder {
				name,
				sd_path: SdPath::Physical {
					device_slug: device_slug.clone(),
					path: spelled,
				},
				path,
			});
		}
		Ok(SystemFoldersOutput { folders })
	}
}

fn display_name(path: &Path) -> String {
	path.file_name()
		.map(|name| name.to_string_lossy().into_owned())
		.unwrap_or_else(|| path.to_string_lossy().into_owned())
}

crate::register_library_query!(PathContextQuery, "paths.context");
crate::register_library_query!(SystemFoldersQuery, "paths.system_folders");

#[cfg(test)]
mod tests {
	use super::*;

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
}
