use crate::{
	invalidate_query,
	job::Job,
	library::LibraryContext,
	object::{
		identifier_job::{FileIdentifierJob, FileIdentifierJobInit},
		preview::{ThumbnailJob, ThumbnailJobInit},
		validation::validator_job::{ObjectValidatorJob, ObjectValidatorJobInit},
	},
	prisma::{indexer_rules_in_location, location, node},
};

use rspc::Type;
use serde::{Deserialize, Serialize};
use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};
use tokio::{
	fs::{self, File},
	io::{self, AsyncWriteExt},
};
use tracing::{debug, info};
use uuid::Uuid;

mod error;
pub mod indexer;
mod manager;

pub use error::LocationError;
use indexer::indexer_job::{indexer_job_location, IndexerJob, IndexerJobInit};
pub use manager::{LocationManager, LocationManagerError};

static LOCATION_METADATA_HIDDEN_DIR: &str = ".spacedrive";

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
		// check if we have access to this location
		if !self.path.try_exists().unwrap() {
			return Err(LocationError::PathNotFound(self.path));
		}

		let path_metadata = fs::metadata(&self.path)
			.await
			.map_err(|e| LocationError::LocationPathMetadataAccess(e, self.path.clone()))?;

		if path_metadata.permissions().readonly() {
			return Err(LocationError::ReadonlyLocationFailure(self.path));
		}

		if !path_metadata.is_dir() {
			return Err(LocationError::NotDirectory(self.path));
		}

		check_or_create_location_metadata_dir(&self.path).await?;

		let metadata_file_name = self
			.path
			.join(LOCATION_METADATA_HIDDEN_DIR)
			.join(format!("{}.json", ctx.id));

		let location_metadata = match fs::read(&metadata_file_name).await {
			Ok(data) => Some(
				serde_json::from_slice::<SpacedriveLocationMetadata>(&data)
					.map_err(|e| LocationError::DotfileSerializeFailure(e, self.path.clone()))?,
			),
			Err(e) if e.kind() == io::ErrorKind::NotFound => None,
			Err(e) => {
				return Err(LocationError::LocationMetadataReadFailure(
					e,
					self.path.clone(),
				));
			}
		};

		let location_name = self.path.file_name().unwrap().to_str().unwrap().to_string();
		let local_path_str = self.path.to_string_lossy().to_string();

		let mut location = if let Some(location_metadata) = location_metadata {
			if location_metadata.library_uuid != ctx.id {
				return Err(LocationError::CorruptedLocationMetadataFile(self.path));
			}
			handle_existing_location_relink(
				location_metadata.location_uuid,
				local_path_str,
				&metadata_file_name,
				ctx,
			)
			.await?
		} else {
			create_new_location(location_name, local_path_str, ctx).await?
		};

		if !self.indexer_rules_ids.is_empty() {
			link_location_and_indexer_rules(ctx, location.id, &self.indexer_rules_ids).await?;
		}

		// Updating our location variable to include information about the indexer rules
		location = fetch_location(ctx, location.id)
			.include(indexer_job_location::include())
			.exec()
			.await?
			.ok_or(LocationError::IdNotFound(location.id))?;

		// Write a location metadata file on a .spacedrive hidden directory with
		// `<library_id>.json` file name containing the location pub id and library id in JSON format
		let mut metadata_file = File::create(metadata_file_name)
			.await
			.map_err(|e| LocationError::LocationMetadataWriteFailure(e, self.path.clone()))?;

		let json_bytes = serde_json::to_vec(&SpacedriveLocationMetadata {
			location_uuid: Uuid::from_slice(&location.pub_id).unwrap(),
			library_uuid: ctx.id,
		})
		.map_err(|e| LocationError::DotfileSerializeFailure(e, self.path.clone()))?;

		metadata_file
			.write_all(&json_bytes)
			.await
			.map_err(|e| LocationError::LocationMetadataWriteFailure(e, self.path))?;

		invalidate_query!(ctx, "locations.list");

		LocationManager::global()
			.add(location.id, ctx.clone())
			.await?;

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

		if location.name != self.name {
			ctx.db
				.location()
				.update(
					location::id::equals(self.id),
					vec![location::name::set(self.name)],
				)
				.exec()
				.await?;
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

#[derive(Serialize, Deserialize, Default)]
pub struct SpacedriveLocationMetadata {
	pub location_uuid: Uuid,
	pub library_uuid: Uuid,
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
	ctx.queue_job(Job::new(
		FileIdentifierJobInit {
			location_id: location.id,
			sub_path: None,
		},
		Box::new(FileIdentifierJob {}),
	))
	.await;
	ctx.spawn_job(Job::new(
		IndexerJobInit { location },
		Box::new(IndexerJob {}),
	))
	.await;
	ctx.queue_job(Job::new(
		ThumbnailJobInit {
			location_id,
			path: PathBuf::new(),
			background: true,
		},
		Box::new(ThumbnailJob {}),
	))
	.await;
	ctx.queue_job(Job::new(
		ObjectValidatorJobInit {
			location_id,
			path: PathBuf::new(),
			background: true,
		},
		Box::new(ObjectValidatorJob {}),
	))
	.await;

	Ok(())
}

async fn handle_existing_location_relink(
	location_pub_id: Uuid,
	local_path_str: String,
	metadata_file_name: impl AsRef<Path>,
	ctx: &LibraryContext,
) -> Result<indexer_job_location::Data, LocationError> {
	let mut location = ctx
		.db
		.location()
		.find_unique(location::pub_id::equals(location_pub_id.as_ref().to_vec()))
		.include(indexer_job_location::include())
		.exec()
		.await?
		.ok_or_else(|| {
			LocationError::LocationMetadataInvalidPubId(
				location_pub_id,
				metadata_file_name.as_ref().to_path_buf(),
			)
		})?;

	if let Some(ref old_local_path) = location.local_path {
		if *old_local_path != local_path_str {
			location = ctx
				.db
				.location()
				.update(
					location::id::equals(location.id),
					vec![
						location::local_path::set(Some(local_path_str)),
						location::is_online::set(true),
					],
				)
				.include(indexer_job_location::include())
				.exec()
				.await?;
		}
	}

	// As we're relinking a location, let's just forget the old indexing rules to receive new ones
	ctx.db
		.indexer_rules_in_location()
		.delete_many(vec![indexer_rules_in_location::location_id::equals(
			location.id,
		)])
		.exec()
		.await?;

	Ok(location)
}

async fn create_new_location(
	location_name: String,
	local_path_str: String,
	ctx: &LibraryContext,
) -> Result<indexer_job_location::Data, LocationError> {
	debug!("Trying to create new location for '{local_path_str}'",);
	let uuid = Uuid::new_v4();

	let location = ctx
		.db
		.location()
		.create(
			uuid.as_bytes().to_vec(),
			node::id::equals(ctx.node_local_id),
			vec![
				location::name::set(Some(location_name)),
				location::is_online::set(true),
				location::local_path::set(Some(local_path_str)),
			],
		)
		.include(indexer_job_location::include())
		.exec()
		.await?;

	info!("Created location: {location:?}");

	Ok(location)
}

async fn check_or_create_location_metadata_dir(
	path: impl AsRef<Path>,
) -> Result<(), LocationError> {
	let metadata_dir_path = path.as_ref().join(LOCATION_METADATA_HIDDEN_DIR);
	(match fs::metadata(&metadata_dir_path).await {
		Ok(_) => Ok(()),
		Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&metadata_dir_path).await,
		Err(e) => Err(e),
	})
	.map_err(|e| LocationError::LocationMetadataDir(e, metadata_dir_path))
}
