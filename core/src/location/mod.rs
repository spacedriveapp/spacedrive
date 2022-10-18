use crate::{
	invalidate_query,
	job::Job,
	library::LibraryContext,
	object::{
		identifier_job::{FileIdentifierJob, FileIdentifierJobInit},
		preview::{ThumbnailJob, ThumbnailJobInit},
	},
	prisma::{indexer_rules_in_location, location, node},
};

use rspc::Type;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
use tokio::{
	fs::{metadata, File},
	io::AsyncWriteExt,
};
use tracing::{debug, info};
use uuid::Uuid;

mod error;
pub mod indexer;

pub use error::LocationError;
use indexer::indexer_job::{IndexerJob, IndexerJobInit};

use self::indexer::indexer_job::indexer_job_location;

static DOTFILE_NAME: &str = ".spacedrive";

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

		let path_metadata = metadata(&self.path)
			.await
			.map_err(|e| LocationError::DotfileReadFailure(e, self.path.clone()))?;

		if path_metadata.permissions().readonly() {
			return Err(LocationError::ReadonlyDotFileLocationFailure(self.path));
		}

		if !path_metadata.is_dir() {
			return Err(LocationError::NotDirectory(self.path));
		}

		// check if the location already exists
		let _location_exists = ctx
			.db
			.location()
			.find_first(vec![location::local_path::equals(Some(
				self.path.to_string_lossy().to_string(),
			))])
			.exec()
			.await?
			.is_some();

		if _location_exists {
			return Err(LocationError::LocationAlreadyExists(self.path));
		}

		debug!(
			"Trying to create new location for '{}'",
			self.path.display()
		);
		let uuid = Uuid::new_v4();

		let mut location = ctx
			.db
			.location()
			.create(
				uuid.as_bytes().to_vec(),
				node::id::equals(ctx.node_local_id),
				vec![
					location::name::set(Some(
						self.path.file_name().unwrap().to_str().unwrap().to_string(),
					)),
					location::is_online::set(true),
					location::local_path::set(Some(self.path.to_string_lossy().to_string())),
				],
			)
			.include(indexer_job_location::include())
			.exec()
			.await?;

		info!("Created location: {:?}", location);

		if !self.indexer_rules_ids.is_empty() {
			link_location_and_indexer_rules(ctx, location.id, &self.indexer_rules_ids).await?;
		}

		// Updating our location variable to include information about the indexer rules
		location = fetch_location(ctx, location.id)
			.include(indexer_job_location::include())
			.exec()
			.await?
			.ok_or(LocationError::IdNotFound(location.id))?;

		// write a file called .spacedrive to path containing the location id in JSON format
		let mut dotfile = File::create(self.path.join(DOTFILE_NAME))
			.await
			.map_err(|e| LocationError::DotfileWriteFailure(e, self.path.clone()))?;

		let json_bytes = serde_json::to_vec(&DotSpacedrive {
			location_uuid: uuid,
			library_uuid: ctx.id,
		})
		.map_err(|e| LocationError::DotfileSerializeFailure(e, self.path.clone()))?;

		dotfile
			.write_all(&json_bytes)
			.await
			.map_err(|e| LocationError::DotfileWriteFailure(e, self.path))?;

		invalidate_query!(ctx, "locations.list");

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
pub struct DotSpacedrive {
	pub location_uuid: Uuid,
	pub library_uuid: Uuid,
}

// checks to see if a location is:
// - accessible on from the local filesystem
// - already exists in the database
// pub async fn check_location(path: &str) -> Result<DotSpacedrive, LocationError> {
// 	let dotfile: DotSpacedrive = match fs::File::open(format!("{}/{}", path.clone(), DOTFILE_NAME))
// 	{
// 		Ok(file) => serde_json::from_reader(file).unwrap_or(DotSpacedrive::default()),
// 		Err(e) => return Err(LocationError::DotfileReadFailure(e)),
// 	};

// 	Ok(dotfile)
// }

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

	Ok(())
}
