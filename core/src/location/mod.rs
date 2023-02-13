use crate::{
	invalidate_query,
	library::LibraryContext,
	object::{
		identifier_job::full_identifier_job::FullFileIdentifierJobInit, preview::ThumbnailJobInit,
	},
	prisma::{file_path, indexer_rules_in_location, location, node, object},
	sync,
};

use rspc::Type;
use serde::Deserialize;
use serde_json::json;
use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};

use prisma_client_rust::QueryError;
use tokio::{fs, io};
use tracing::{debug, info};
use uuid::Uuid;

mod error;
pub mod file_path_helper;
pub mod indexer;
mod manager;
mod metadata;

pub use error::LocationError;
use indexer::indexer_job::{indexer_job_location, IndexerJobInit};
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

		// Write a location metadata on a .spacedrive file
		SpacedriveLocationMetadataFile::create_and_save(
			ctx.id,
			uuid,
			&self.path,
			location.name.as_ref().unwrap().clone(),
		)
		.await?;

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
	pub generate_preview_media: Option<bool>,
	pub sync_preview_media: Option<bool>,
	pub hidden: Option<bool>,
	pub indexer_rules_ids: Vec<i32>,
}

impl LocationUpdateArgs {
	pub async fn update(self, ctx: &LibraryContext) -> Result<(), LocationError> {
		let location = fetch_location(ctx, self.id)
			.include(location::include!({ indexer_rules }))
			.exec()
			.await?
			.ok_or(LocationError::IdNotFound(self.id))?;

		let params = [
			self.name
				.clone()
				.filter(|name| location.name.as_ref() != Some(name))
				.map(|v| location::name::set(Some(v))),
			self.generate_preview_media
				.map(location::generate_preview_media::set),
			self.sync_preview_media
				.map(location::sync_preview_media::set),
			self.hidden.map(location::hidden::set),
		]
		.into_iter()
		.flatten()
		.collect::<Vec<_>>();

		if !params.is_empty() {
			ctx.db
				.location()
				.update(location::id::equals(self.id), params)
				.exec()
				.await?;

			if let Some(ref local_path) = location.local_path {
				if let Some(mut metadata) =
					SpacedriveLocationMetadataFile::try_load(local_path).await?
				{
					metadata.update(ctx.id, self.name.unwrap()).await?;
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

	let location_id = location.id;

	// TODO: This code makes the assumption that their is a single worker thread. This is true today but may not be true in the future refactor to not make that assumption.
	ctx.spawn_job(IndexerJobInit { location }).await;
	ctx.spawn_job(FullFileIdentifierJobInit {
		location_id: location_id,
		sub_path: None,
	})
	.await;
	ctx.spawn_job(ThumbnailJobInit {
		location_id: location_id,
		root_path: PathBuf::new(),
		background: false,
	})
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
			vec![location::local_path::set(Some(
				location_path
					.as_ref()
					.to_str()
					.expect("Found non-UTF-8 path")
					.to_string(),
			))],
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
	let db = &ctx.db;

	let location_name = location_path
		.as_ref()
		.file_name()
		.unwrap()
		.to_str()
		.unwrap()
		.to_string();

	let local_path = location_path
		.as_ref()
		.to_str()
		.expect("Found non-UTF-8 path")
		.to_string();

	let location = ctx
		.sync
		.write_op(
			db,
			ctx.sync.owned_create(
				sync::location::SyncId {
					pub_id: location_pub_id.as_bytes().to_vec(),
				},
				[
					("node", json!({ "pub_id": ctx.id.as_bytes() })),
					("name", json!(location_name)),
					("local_path", json!(&local_path)),
				],
			),
			db.location()
				.create(
					location_pub_id.as_bytes().to_vec(),
					node::id::equals(ctx.node_local_id),
					vec![
						location::name::set(Some(location_name.clone())),
						location::local_path::set(Some(local_path)),
					],
				)
				.include(indexer_job_location::include()),
		)
		.await?;

	if !indexer_rules_ids.is_empty() {
		link_location_and_indexer_rules(ctx, location.id, indexer_rules_ids).await?;
	}

	// Updating our location variable to include information about the indexer rules
	let location = fetch_location(ctx, location.id)
		.include(indexer_job_location::include())
		.exec()
		.await?
		.ok_or(LocationError::IdNotFound(location.id))?;

	invalidate_query!(ctx, "locations.list");

	ctx.location_manager().add(location.id, ctx.clone()).await?;

	Ok(location)
}

pub async fn delete_location(ctx: &LibraryContext, location_id: i32) -> Result<(), LocationError> {
	let LibraryContext { db, .. } = ctx;

	ctx.location_manager()
		.remove(location_id, ctx.clone())
		.await?;

	delete_directory(ctx, location_id, None).await?;

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

file_path::select!(file_path_object_id_only { object_id });

/// Will delete a directory recursively with Objects if left as orphans
/// this function is used to delete a location and when ingesting directory deletion events
pub async fn delete_directory(
	ctx: &LibraryContext,
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
	let object_ids = ctx
		.db
		.file_path()
		.find_many(children_params.clone())
		.select(file_path_object_id_only::select())
		.exec()
		.await?
		.into_iter()
		.filter_map(|file_path| file_path.object_id)
		.collect();

	// WARNING: file_paths must be deleted before objects, as they reference objects through object_id
	// delete all children file_paths
	ctx.db
		.file_path()
		.delete_many(children_params)
		.exec()
		.await?;

	// delete all children objects
	ctx.db
		.object()
		.delete_many(vec![
			object::id::in_vec(object_ids),
			// https://www.prisma.io/docs/reference/api-reference/prisma-client-reference#none
			object::file_paths::none(vec![]),
		])
		.exec()
		.await?;

	invalidate_query!(ctx, "locations.getExplorerData");

	Ok(())
}

// check if a path exists in our database at that location
pub async fn check_virtual_path_exists(
	library_ctx: &LibraryContext,
	location_id: i32,
	subpath: impl AsRef<Path>,
) -> Result<bool, LocationError> {
	let path = subpath.as_ref().to_str().unwrap().to_string();

	let file_path = library_ctx
		.db
		.file_path()
		.find_first(vec![
			file_path::location_id::equals(location_id),
			file_path::materialized_path::equals(path),
		])
		.exec()
		.await?;

	Ok(file_path.is_some())
}
