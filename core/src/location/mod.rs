use crate::{
	invalidate_query,
	job::{Job, JobManagerError},
	library::Library,
	object::{
		file_identifier::{
			file_identifier_job::FileIdentifierJobInit,
			shallow_file_identifier_job::ShallowFileIdentifierJobInit,
		},
		preview::{
			shallow_thumbnailer_job::ShallowThumbnailerJobInit, thumbnailer_job::ThumbnailerJobInit,
		},
	},
	prisma::{file_path, indexer_rules_in_location, location, node, object},
	sync,
};

use std::{
	collections::HashSet,
	path::{Component, Path, PathBuf},
};

use futures::future::TryFutureExt;
use normpath::PathExt;
use prisma_client_rust::QueryError;
use rspc::Type;
use serde::Deserialize;
use serde_json::json;
use tokio::{fs, io};
use tracing::{debug, info};
use uuid::Uuid;

mod error;
pub mod file_path_helper;
pub mod indexer;
mod manager;
mod metadata;

pub use error::LocationError;
use file_path_helper::file_path_just_object_id;
use indexer::{shallow_indexer_job::ShallowIndexerJobInit, IndexerJobInit};
pub use manager::{LocationManager, LocationManagerError};
use metadata::SpacedriveLocationMetadataFile;

pub type LocationId = i32;

// Location includes!
location::include!(location_with_indexer_rules {
	indexer_rules: select { indexer_rule }
});

/// `LocationCreateArgs` is the argument received from the client using `rspc` to create a new location.
/// It has the actual path and a vector of indexer rules ids, to create many-to-many relationships
/// between the location and indexer rules.
#[derive(Type, Deserialize)]
pub struct LocationCreateArgs {
	pub path: PathBuf,
	pub indexer_rules_ids: Vec<i32>,
}

impl LocationCreateArgs {
	pub async fn create(
		self,
		library: &Library,
	) -> Result<location_with_indexer_rules::Data, LocationError> {
		let path_metadata = match fs::metadata(&self.path).await {
			Ok(metadata) => metadata,
			Err(e) if e.kind() == io::ErrorKind::NotFound => {
				return Err(LocationError::PathNotFound(self.path))
			}
			Err(e) => {
				return Err(LocationError::LocationPathFilesystemMetadataAccess(
					e, self.path,
				));
			}
		};

		if !path_metadata.is_dir() {
			return Err(LocationError::NotDirectory(self.path));
		}

		if let Some(metadata) = SpacedriveLocationMetadataFile::try_load(&self.path).await? {
			return if metadata.has_library(library.id) {
				Err(LocationError::NeedRelink {
					// SAFETY: This unwrap is ok as we checked that we have this library_id
					old_path: metadata.location_path(library.id).unwrap().to_path_buf(),
					new_path: self.path,
				})
			} else {
				Err(LocationError::AddLibraryToMetadata(self.path))
			};
		}

		debug!(
			"Trying to create new location for '{}'",
			self.path.display()
		);
		let uuid = Uuid::new_v4();

		let location = create_location(library, uuid, &self.path, &self.indexer_rules_ids).await?;

		// Write location metadata to a .spacedrive file
		if let Err(err) = SpacedriveLocationMetadataFile::create_and_save(
			library.id,
			uuid,
			&self.path,
			location.name.clone(),
		)
		.err_into::<LocationError>()
		.and_then(|()| async move {
			Ok(library
				.location_manager()
				.add(location.id, library.clone())
				.await?)
		})
		.await
		{
			delete_location(library, location.id).await?;
			Err(err)?;
		}

		info!("Created location: {location:?}");

		Ok(location)
	}

	pub async fn add_library(
		self,
		library: &Library,
	) -> Result<location_with_indexer_rules::Data, LocationError> {
		let mut metadata = SpacedriveLocationMetadataFile::try_load(&self.path)
			.await?
			.ok_or_else(|| LocationError::MetadataNotFound(self.path.clone()))?;

		if metadata.has_library(library.id) {
			return Err(LocationError::NeedRelink {
				// SAFETY: This unwrap is ok as we checked that we have this library_id
				old_path: metadata.location_path(library.id).unwrap().to_path_buf(),
				new_path: self.path,
			});
		}

		debug!(
			"Trying to add a new library (library_id = {}) to an already existing location '{}'",
			library.id,
			self.path.display()
		);

		let uuid = Uuid::new_v4();

		let location = create_location(library, uuid, &self.path, &self.indexer_rules_ids).await?;

		metadata
			.add_library(library.id, uuid, &self.path, location.name.clone())
			.await?;

		library
			.location_manager()
			.add(location.id, library.clone())
			.await?;

		info!(
			"Added library (library_id = {}) to location: {location:?}",
			library.id
		);

		Ok(location)
	}
}

/// `LocationUpdateArgs` is the argument received from the client using `rspc` to update a location.
/// It contains the id of the location to be updated, possible a name to change the current location's name
/// and a vector of indexer rules ids to add or remove from the location.
///
/// It is important to note that only the indexer rule ids in this vector will be used from now on.
/// Old rules that aren't in this vector will be purged.
#[derive(Type, Deserialize)]
pub struct LocationUpdateArgs {
	pub id: i32,
	pub name: Option<String>,
	pub generate_preview_media: Option<bool>,
	pub sync_preview_media: Option<bool>,
	pub hidden: Option<bool>,
	pub indexer_rules_ids: Vec<i32>,
}

impl LocationUpdateArgs {
	pub async fn update(self, library: &Library) -> Result<(), LocationError> {
		let Library { sync, db, .. } = &library;

		let location = find_location(library, self.id)
			.include(location_with_indexer_rules::include())
			.exec()
			.await?
			.ok_or(LocationError::IdNotFound(self.id))?;

		let (sync_params, db_params): (Vec<_>, Vec<_>) = [
			self.name
				.clone()
				.filter(|name| &location.name != name)
				.map(|v| (("name", json!(v)), location::name::set(v))),
			self.generate_preview_media.map(|v| {
				(
					("generate_preview_media", json!(v)),
					location::generate_preview_media::set(v),
				)
			}),
			self.sync_preview_media.map(|v| {
				(
					("sync_preview_media", json!(v)),
					location::sync_preview_media::set(v),
				)
			}),
			self.hidden
				.map(|v| (("hidden", json!(v)), location::hidden::set(v))),
		]
		.into_iter()
		.flatten()
		.unzip();

		if !sync_params.is_empty() {
			sync.write_ops(
				db,
				(
					sync_params
						.into_iter()
						.map(|p| {
							sync.shared_update(
								sync::location::SyncId {
									pub_id: location.pub_id.clone(),
								},
								p.0,
								p.1,
							)
						})
						.collect(),
					db.location()
						.update(location::id::equals(self.id), db_params),
				),
			)
			.await?;

			if location.node_id == library.node_local_id {
				if let Some(mut metadata) =
					SpacedriveLocationMetadataFile::try_load(&location.path).await?
				{
					metadata.update(library.id, self.name.unwrap()).await?;
				}
			}
		}

		let current_rules_ids = location
			.indexer_rules
			.iter()
			.map(|r| r.indexer_rule.id)
			.collect::<HashSet<_>>();

		let new_rules_ids = self.indexer_rules_ids.into_iter().collect::<HashSet<_>>();

		if current_rules_ids != new_rules_ids {
			let rule_ids_to_add = new_rules_ids
				.difference(&current_rules_ids)
				.copied()
				.collect::<Vec<_>>();
			let rule_ids_to_remove = current_rules_ids
				.difference(&new_rules_ids)
				.copied()
				.collect::<Vec<_>>();

			if !rule_ids_to_remove.is_empty() {
				library
					.db
					.indexer_rules_in_location()
					.delete_many(vec![
						indexer_rules_in_location::location_id::equals(self.id),
						indexer_rules_in_location::indexer_rule_id::in_vec(rule_ids_to_remove),
					])
					.exec()
					.await?;
			}

			if !rule_ids_to_add.is_empty() {
				link_location_and_indexer_rules(library, self.id, &rule_ids_to_add).await?;
			}
		}

		Ok(())
	}
}

pub fn find_location(library: &Library, location_id: i32) -> location::FindUnique {
	library
		.db
		.location()
		.find_unique(location::id::equals(location_id))
}

async fn link_location_and_indexer_rules(
	library: &Library,
	location_id: i32,
	rules_ids: &[i32],
) -> Result<(), LocationError> {
	library
		.db
		.indexer_rules_in_location()
		.create_many(
			rules_ids
				.iter()
				.map(|id| indexer_rules_in_location::create_unchecked(location_id, *id, vec![]))
				.collect(),
		)
		.exec()
		.await?;

	Ok(())
}

pub async fn scan_location(
	library: &Library,
	location: location_with_indexer_rules::Data,
) -> Result<(), JobManagerError> {
	if location.node_id != library.node_local_id {
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	library
		.spawn_job(
			Job::new(IndexerJobInit {
				location,
				sub_path: None,
			})
			.queue_next(FileIdentifierJobInit {
				location: location_base_data.clone(),
				sub_path: None,
			})
			.queue_next(ThumbnailerJobInit {
				location: location_base_data,
				sub_path: None,
				background: true,
			}),
		)
		.await
}

#[cfg(feature = "location-watcher")]
pub async fn scan_location_sub_path(
	library: &Library,
	location: location_with_indexer_rules::Data,
	sub_path: impl AsRef<Path>,
) -> Result<(), JobManagerError> {
	let sub_path = sub_path.as_ref().to_path_buf();
	if location.node_id != library.node_local_id {
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	library
		.spawn_job(
			Job::new(IndexerJobInit {
				location,
				sub_path: Some(sub_path.clone()),
			})
			.queue_next(FileIdentifierJobInit {
				location: location_base_data.clone(),
				sub_path: Some(sub_path.clone()),
			})
			.queue_next(ThumbnailerJobInit {
				location: location_base_data,
				sub_path: Some(sub_path),
				background: true,
			}),
		)
		.await
}

pub async fn light_scan_location(
	library: &Library,
	location: location_with_indexer_rules::Data,
	sub_path: impl AsRef<Path>,
) -> Result<(), JobManagerError> {
	let sub_path = sub_path.as_ref().to_path_buf();
	if location.node_id != library.node_local_id {
		return Ok(());
	}

	let location_base_data = location::Data::from(&location);

	library
		.spawn_job(
			Job::new(ShallowIndexerJobInit {
				location,
				sub_path: sub_path.clone(),
			})
			.queue_next(ShallowFileIdentifierJobInit {
				location: location_base_data.clone(),
				sub_path: sub_path.clone(),
			})
			.queue_next(ShallowThumbnailerJobInit {
				location: location_base_data,
				sub_path,
			}),
		)
		.await
}

pub async fn relink_location(
	library: &Library,
	location_path: impl AsRef<Path>,
) -> Result<(), LocationError> {
	let Library { db, id, sync, .. } = &library;

	let mut metadata = SpacedriveLocationMetadataFile::try_load(&location_path)
		.await?
		.ok_or_else(|| LocationError::MissingMetadataFile(location_path.as_ref().to_path_buf()))?;

	metadata.relink(*id, &location_path).await?;

	let pub_id = metadata.location_pub_id(library.id)?.as_ref().to_vec();
	let path = location_path
		.as_ref()
		.to_str()
		.expect("Found non-UTF-8 path")
		.to_string();

	sync.write_op(
		db,
		sync.shared_update(
			sync::location::SyncId {
				pub_id: pub_id.clone(),
			},
			"path",
			json!(path),
		),
		db.location().update(
			location::pub_id::equals(pub_id),
			vec![location::path::set(path)],
		),
	)
	.await?;

	Ok(())
}

async fn create_location(
	library: &Library,
	location_pub_id: Uuid,
	location_path: impl AsRef<Path>,
	indexer_rules_ids: &[i32],
) -> Result<location_with_indexer_rules::Data, LocationError> {
	let Library { db, sync, .. } = &library;

	let location_path = location_path.as_ref();

	let name = location_path.normalize().map_or_else(
		|_| {
			location_path
				.components()
				.next()
				.and_then(|component| match component {
					Component::Prefix(component) => {
						let prefix = component.as_os_str().to_string_lossy().to_string();
						Some(if prefix == "/" {
							"Root".to_string()
						} else {
							prefix
						})
					}
					_ => None,
				})
				.get_or_insert("Unknown".to_string())
				.to_owned()
		},
		|p| p.localize_name().to_string_lossy().to_string(),
	);

	let path = location_path
		.to_str()
		.map(str::to_string)
		.expect("Found non-UTF-8 path");

	let location = sync
		.write_op(
			db,
			sync.unique_shared_create(
				sync::location::SyncId {
					pub_id: location_pub_id.as_bytes().to_vec(),
				},
				[
					("node", json!({ "pub_id": library.id.as_bytes() })),
					("name", json!(&name)),
					("path", json!(&path)),
				],
			),
			db.location()
				.create(
					location_pub_id.as_bytes().to_vec(),
					name,
					path,
					node::id::equals(library.node_local_id),
					vec![],
				)
				.include(location_with_indexer_rules::include()),
		)
		.await?;

	debug!("created in db");

	if !indexer_rules_ids.is_empty() {
		link_location_and_indexer_rules(library, location.id, indexer_rules_ids).await?;
	}

	// Updating our location variable to include information about the indexer rules
	let location = find_location(library, location.id)
		.include(location_with_indexer_rules::include())
		.exec()
		.await?
		.ok_or(LocationError::IdNotFound(location.id))?;

	invalidate_query!(library, "locations.list");

	Ok(location)
}

pub async fn delete_location(library: &Library, location_id: i32) -> Result<(), LocationError> {
	let Library { db, .. } = library;

	library
		.location_manager()
		.remove(location_id, library.clone())
		.await?;

	delete_directory(library, location_id, None).await?;

	db.indexer_rules_in_location()
		.delete_many(vec![indexer_rules_in_location::location_id::equals(
			location_id,
		)])
		.exec()
		.await?;

	let location = db
		.location()
		.delete(location::id::equals(location_id))
		.exec()
		.await?;

	if location.node_id == library.node_local_id {
		if let Ok(Some(mut metadata)) =
			SpacedriveLocationMetadataFile::try_load(&location.path).await
		{
			metadata.remove_library(library.id).await?;
		}
	}

	info!("Location {} deleted", location_id);
	invalidate_query!(library, "locations.list");

	Ok(())
}

/// Will delete a directory recursively with Objects if left as orphans
/// this function is used to delete a location and when ingesting directory deletion events
pub async fn delete_directory(
	library: &Library,
	location_id: i32,
	parent_materialized_path: Option<String>,
) -> Result<(), QueryError> {
	let children_params = if let Some(parent_materialized_path) = parent_materialized_path {
		vec![
			file_path::location_id::equals(location_id),
			file_path::materialized_path::starts_with(parent_materialized_path),
		]
	} else {
		vec![file_path::location_id::equals(location_id)]
	};

	// Fetching all object_ids from all children file_paths
	let object_ids = library
		.db
		.file_path()
		.find_many(children_params.clone())
		.select(file_path_just_object_id::select())
		.exec()
		.await?
		.into_iter()
		.filter_map(|file_path| file_path.object_id)
		.collect();

	// WARNING: file_paths must be deleted before objects, as they reference objects through object_id
	// delete all children file_paths
	library
		.db
		.file_path()
		.delete_many(children_params)
		.exec()
		.await?;

	// delete all children objects
	library
		.db
		.object()
		.delete_many(vec![
			object::id::in_vec(object_ids),
			// https://www.prisma.io/docs/reference/api-reference/prisma-client-reference#none
			object::file_paths::none(vec![]),
		])
		.exec()
		.await?;

	invalidate_query!(library, "locations.getExplorerData");

	Ok(())
}

impl From<location_with_indexer_rules::Data> for location::Data {
	fn from(data: location_with_indexer_rules::Data) -> Self {
		Self {
			id: data.id,
			pub_id: data.pub_id,
			path: data.path,
			node_id: data.node_id,
			name: data.name,
			total_capacity: data.total_capacity,
			available_capacity: data.available_capacity,
			is_archived: data.is_archived,
			generate_preview_media: data.generate_preview_media,
			sync_preview_media: data.sync_preview_media,
			hidden: data.hidden,
			date_created: data.date_created,
			node: None,
			file_paths: None,
			indexer_rules: None,
		}
	}
}

impl From<&location_with_indexer_rules::Data> for location::Data {
	fn from(data: &location_with_indexer_rules::Data) -> Self {
		Self {
			id: data.id,
			pub_id: data.pub_id.clone(),
			path: data.path.clone(),
			node_id: data.node_id,
			name: data.name.clone(),
			total_capacity: data.total_capacity,
			available_capacity: data.available_capacity,
			is_archived: data.is_archived,
			generate_preview_media: data.generate_preview_media,
			sync_preview_media: data.sync_preview_media,
			hidden: data.hidden,
			date_created: data.date_created,
			node: None,
			file_paths: None,
			indexer_rules: None,
		}
	}
}

// check if a path exists in our database at that location
// pub async fn check_virtual_path_exists(
// 	library: &Library,
// 	location_id: i32,
// 	subpath: impl AsRef<Path>,
// ) -> Result<bool, LocationError> {
// 	let path = subpath.as_ref().to_str().unwrap().to_string();

// 	let file_path = library
// 		.db
// 		.file_path()
// 		.find_first(vec![
// 			file_path::location_id::equals(location_id),
// 			file_path::materialized_path::equals(path),
// 		])
// 		.exec()
// 		.await?;

// 	Ok(file_path.is_some())
// }
