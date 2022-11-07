use crate::{
	invalidate_query,
	job::Job,
	library::LibraryContext,
	object::{
		identifier_job::full_identifier_job::{FullFileIdentifierJob, FullFileIdentifierJobInit},
		preview::{ThumbnailJob, ThumbnailJobInit},
	},
	prisma::{file_path, indexer_rules_in_location, location, node, object},
};

use prisma_client_rust::Direction;
use rspc::Type;
use serde::Deserialize;
use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};
use tokio::{fs, io};
use tracing::{debug, error, info};
use uuid::Uuid;

mod error;
pub mod indexer;
mod manager;
mod metadata;

pub use error::LocationError;
use indexer::indexer_job::{indexer_job_location, IndexerJob, IndexerJobInit};
pub use manager::{LocationManager, LocationManagerError};
use metadata::SpacedriveLocationMetadataFile;

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
		ctx: &LibraryContext,
	) -> Result<indexer_job_location::Data, LocationError> {
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

		if path_metadata.permissions().readonly() {
			return Err(LocationError::ReadonlyLocationFailure(self.path));
		}

		if !path_metadata.is_dir() {
			return Err(LocationError::NotDirectory(self.path));
		}

		if let Some(metadata) = SpacedriveLocationMetadataFile::try_load(&self.path).await? {
			return if metadata.has_library(ctx.id) {
				Err(LocationError::NeedRelink {
					// SAFETY: This unwrap is ok as we checked that we have this library_id
					old_path: metadata.location_path(ctx.id).unwrap().to_path_buf(),
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

		let location = create_location(ctx, uuid, &self.path, &self.indexer_rules_ids).await?;

		// // Write a location metadata on a .spacedrive file
		// SpacedriveLocationMetadataFile::create_and_save(
		// 	ctx.id,
		// 	uuid,
		// 	&self.path,
		// 	location.name.as_ref().unwrap().clone(),
		// )
		// .await?;
		// Jamie: turning this off because it is hell until it works properly

		info!("Created location: {location:?}");

		Ok(location)
	}

	pub async fn add_library(
		self,
		ctx: &LibraryContext,
	) -> Result<indexer_job_location::Data, LocationError> {
		let mut metadata = SpacedriveLocationMetadataFile::try_load(&self.path)
			.await?
			.ok_or_else(|| LocationError::MetadataNotFound(self.path.clone()))?;

		if metadata.has_library(ctx.id) {
			return Err(LocationError::NeedRelink {
				// SAFETY: This unwrap is ok as we checked that we have this library_id
				old_path: metadata.location_path(ctx.id).unwrap().to_path_buf(),
				new_path: self.path,
			});
		}

		debug!(
			"Trying to add a new library (library_id = {}) to an already existing location '{}'",
			ctx.id,
			self.path.display()
		);

		let uuid = Uuid::new_v4();

		let location = create_location(ctx, uuid, &self.path, &self.indexer_rules_ids).await?;

		metadata
			.add_library(
				ctx.id,
				uuid,
				&self.path,
				location.name.as_ref().unwrap().clone(),
			)
			.await?;

		info!(
			"Added library (library_id = {}) to location: {location:?}",
			ctx.id
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
	pub indexer_rules_ids: Vec<i32>,
}

impl LocationUpdateArgs {
	pub async fn update(self, ctx: &LibraryContext) -> Result<(), LocationError> {
		let location = fetch_location(ctx, self.id)
			.include(location::include!({ indexer_rules }))
			.exec()
			.await?
			.ok_or(LocationError::IdNotFound(self.id))?;

		if self.name.is_some() && location.name != self.name {
			ctx.db
				.location()
				.update(
					location::id::equals(self.id),
					vec![location::name::set(self.name.clone())],
				)
				.exec()
				.await?;

			if let Some(ref local_path) = location.local_path {
				if let Some(mut metadata) =
					SpacedriveLocationMetadataFile::try_load(local_path).await?
				{
					metadata.update(ctx.id, self.name.unwrap().clone()).await?;
				}
			}
		}

		let current_rules_ids = location
			.indexer_rules
			.iter()
			.map(|r| r.indexer_rule_id)
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
				ctx.db
					.indexer_rules_in_location()
					.delete_many(vec![
						indexer_rules_in_location::location_id::equals(self.id),
						indexer_rules_in_location::indexer_rule_id::in_vec(rule_ids_to_remove),
					])
					.exec()
					.await?;
			}

			if !rule_ids_to_add.is_empty() {
				link_location_and_indexer_rules(ctx, self.id, &rule_ids_to_add).await?;
			}
		}

		Ok(())
	}
}

async fn get_location(location_id: i32, library_ctx: &LibraryContext) -> Option<location::Data> {
	library_ctx
		.db
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await
		.unwrap_or_else(|err| {
			error!("Failed to get location data from location_id: {:#?}", err);
			None
		})
}

pub fn fetch_location(ctx: &LibraryContext, location_id: i32) -> location::FindUnique {
	ctx.db
		.location()
		.find_unique(location::id::equals(location_id))
}

async fn link_location_and_indexer_rules(
	ctx: &LibraryContext,
	location_id: i32,
	rules_ids: &[i32],
) -> Result<(), LocationError> {
	ctx.db
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
	ctx: &LibraryContext,
	location: indexer_job_location::Data,
) -> Result<(), LocationError> {
	if location.local_path.is_none() {
		return Err(LocationError::MissingLocalPath(location.id));
	};

	ctx.queue_job(Job::new(
		FullFileIdentifierJobInit {
			location_id: location.id,
			sub_path: None,
		},
		FullFileIdentifierJob {},
	))
	.await;

	ctx.queue_job(Job::new(
		ThumbnailJobInit {
			location_id: location.id,
			root_path: PathBuf::new(),
			background: true,
		},
		ThumbnailJob {},
	))
	.await;

	ctx.spawn_job(Job::new(IndexerJobInit { location }, IndexerJob {}))
		.await;

	Ok(())
}

pub async fn relink_location(
	ctx: &LibraryContext,
	location_path: impl AsRef<Path>,
) -> Result<(), LocationError> {
	let mut metadata = SpacedriveLocationMetadataFile::try_load(&location_path)
		.await?
		.ok_or_else(|| LocationError::MissingMetadataFile(location_path.as_ref().to_path_buf()))?;

	metadata.relink(ctx.id, &location_path).await?;

	ctx.db
		.location()
		.update(
			location::pub_id::equals(metadata.location_pub_id(ctx.id)?.as_ref().to_vec()),
			vec![
				location::local_path::set(Some(
					location_path.as_ref().to_string_lossy().to_string(),
				)),
				location::is_online::set(true),
			],
		)
		.exec()
		.await?;

	Ok(())
}

async fn create_location(
	ctx: &LibraryContext,
	location_pub_id: Uuid,
	location_path: impl AsRef<Path>,
	indexer_rules_ids: &[i32],
) -> Result<indexer_job_location::Data, LocationError> {
	let location_name = location_path
		.as_ref()
		.file_name()
		.unwrap()
		.to_str()
		.unwrap()
		.to_string();

	let mut location = ctx
		.db
		.location()
		.create(
			location_pub_id.as_bytes().to_vec(),
			node::id::equals(ctx.node_local_id),
			vec![
				location::name::set(Some(location_name.clone())),
				location::is_online::set(true),
				location::local_path::set(Some(
					location_path.as_ref().to_string_lossy().to_string(),
				)),
			],
		)
		.include(indexer_job_location::include())
		.exec()
		.await?;

	if !indexer_rules_ids.is_empty() {
		link_location_and_indexer_rules(ctx, location.id, indexer_rules_ids).await?;
	}

	// Updating our location variable to include information about the indexer rules
	location = fetch_location(ctx, location.id)
		.include(indexer_job_location::include())
		.exec()
		.await?
		.ok_or(LocationError::IdNotFound(location.id))?;

	invalidate_query!(ctx, "locations.list");

	LocationManager::global()
		.add(location.id, ctx.clone())
		.await?;

	Ok(location)
}

pub async fn delete_location(ctx: &LibraryContext, location_id: i32) -> Result<(), LocationError> {
	delete_directory(ctx, location_id, None).await?;

	ctx.db
		.indexer_rules_in_location()
		.delete_many(vec![indexer_rules_in_location::location_id::equals(
			location_id,
		)])
		.exec()
		.await?;

	let location = ctx
		.db
		.location()
		.delete(location::id::equals(location_id))
		.exec()
		.await?;

	if let Err(e) = LocationManager::global().remove(location_id).await {
		error!("Failed to remove location from manager: {e:#?}");
	}

	if let Some(local_path) = location.local_path {
		if let Ok(Some(mut metadata)) = SpacedriveLocationMetadataFile::try_load(&local_path).await
		{
			metadata.remove_library(ctx.id).await?;
		}
	}

	info!("Location {} deleted", location_id);
	invalidate_query!(ctx, "locations.list");

	Ok(())
}

/// Will delete a directory recursively with Objects if left as orphans
/// this function is used to delete a location and when ingesting directory deletion events
///
/// TODO: find a way to delete orphaned objects
pub async fn delete_directory(
	ctx: &LibraryContext,
	location_id: i32,
	directory_path: Option<String>,
) -> Result<(), LocationError> {
	// get directory
	let mut params = vec![file_path::location_id::equals(location_id)];
	if let Some(directory_path) = directory_path {
		params.push(file_path::materialized_path::equals(directory_path.clone()));
	}
	let directory = ctx.db.file_path().find_first(params).exec().await?;

	if let Some(directory) = directory {
		// get all file_paths starting with the directory path
		let file_paths = ctx
			.db
			.file_path()
			.find_many(vec![
				file_path::location_id::equals(location_id),
				file_path::materialized_path::starts_with(directory.materialized_path),
			])
			.exec()
			.await?;

		// let object_ids: Vec<i32> = file_paths
		// 	.iter()
		// 	.filter_map(|file_path| file_path.object_id)
		// 	.collect();

		// map file paths into array of ids
		let file_path_ids: Vec<i32> = file_paths.iter().map(|f| f.id).collect();

		// delete all file paths
		ctx.db
			.file_path()
			.delete_many(vec![file_path::id::in_vec(file_path_ids)])
			.exec()
			.await?;
	} else {
		return Err(LocationError::DirectoryNotFound("idk".to_string()));
	}
	Ok(())
}

fn strip_location_root_path(
	location_path: impl AsRef<Path>,
	current_path: impl AsRef<Path>,
) -> Option<PathBuf> {
	let location_path = location_path.as_ref();
	let current_path = current_path.as_ref();

	if let Ok(stripped) = current_path.strip_prefix(location_path) {
		Some(stripped.to_path_buf())
	} else {
		error!(
			"Failed to strip location root path ({}) from current path ({})",
			location_path.display(),
			current_path.display()
		);
		None
	}
}

// not sure if needed
fn strip_location_root_path_and_filename(
	location_path: impl AsRef<Path>,
	current_path: impl AsRef<Path>,
) -> Option<PathBuf> {
	let location_path = location_path.as_ref();
	let current_path = current_path.as_ref();

	if let Ok(stripped) = current_path.strip_prefix(location_path) {
		if let Some(parent) = stripped.parent() {
			return Some(parent.to_path_buf());
		}
	}
	error!(
		"Failed to strip location root path ({}) and filename from current path ({})",
		location_path.display(),
		current_path.display()
	);

	None
}

file_path::select!(file_path_id_only { id });

pub async fn get_max_file_path_id(ctx: &LibraryContext) -> Result<i32, LocationError> {
	Ok(ctx
		.db
		.file_path()
		.find_first(vec![])
		.order_by(file_path::id::order(Direction::Desc))
		.select(file_path_id_only::select())
		.exec()
		.await?
		.map(|r| r.id)
		.unwrap_or(0))
}
