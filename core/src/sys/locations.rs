use crate::{
	api::LibraryArgs,
	encode::{ThumbnailJob, ThumbnailJobInit},
	file::{
		cas::{FileIdentifierJob, FileIdentifierJobInit},
		indexer::{IndexerJob, IndexerJobInit},
	},
	invalidate_query,
	job::Job,
	library::LibraryContext,
	prisma::{self, location},
};

use rspc::ErrorCode;
use serde::{Deserialize, Serialize};
use std::{
	fmt::Debug,
	path::{Path, PathBuf},
};
use thiserror::Error;
use tokio::{
	fs::{metadata, File},
	io::{self, AsyncWriteExt},
};
use tracing::info;

use uuid::Uuid;

#[derive(Serialize, Deserialize, Default)]
pub struct DotSpacedrive {
	pub location_uuid: Uuid,
	pub library_uuid: Uuid,
}

static DOTFILE_NAME: &str = ".spacedrive";

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

pub async fn scan_location(ctx: &LibraryContext, location_id: i32, path: impl AsRef<Path>) {
	let path_buf = path.as_ref().to_path_buf();
	ctx.spawn_job(Job::new(
		IndexerJobInit {
			path: path_buf.clone(),
		},
		Box::new(IndexerJob {}),
	))
	.await;
	ctx.queue_job(Job::new(
		FileIdentifierJobInit {
			location_id,
			path: path_buf.clone(),
		},
		Box::new(FileIdentifierJob {}),
	))
	.await;

	ctx.queue_job(Job::new(
		ThumbnailJobInit {
			location_id,
			path: path_buf,
			background: true,
		},
		Box::new(ThumbnailJob {}),
	))
	.await;
}

pub async fn create_location(
	library: &LibraryContext,
	path: impl AsRef<Path> + Debug,
) -> Result<location::Data, LocationError> {
	let path = path.as_ref();

	// check if we have access to this location
	if !path.exists() {
		return Err(LocationError::PathNotFound(path.to_owned()));
	}

	if metadata(path)
		.await
		.map_err(|e| LocationError::DotfileReadFailure(e, path.to_owned()))?
		.permissions()
		.readonly()
	{
		return Err(LocationError::ReadonlyDotFileLocationFailure(
			path.to_owned(),
		));
	}

	let path_string = path.to_str().unwrap().to_string();

	// check if location already exists
	let location_resource = if let Some(location) = library
		.db
		.location()
		.find_first(vec![location::local_path::equals(Some(
			path_string.clone(),
		))])
		.exec()
		.await?
	{
		location
	} else {
		info!(
			"Location does not exist, creating new location for '{}'",
			path_string
		);
		let uuid = Uuid::new_v4();

		let location = library
			.db
			.location()
			.create(
				uuid.as_bytes().to_vec(),
				vec![
					location::name::set(Some(
						path.file_name().unwrap().to_str().unwrap().to_string(),
					)),
					location::is_online::set(true),
					location::local_path::set(Some(path_string)),
					location::node_id::set(Some(library.node_local_id)),
				],
			)
			.exec()
			.await?;

		info!("Created location: {:?}", location);

		// write a file called .spacedrive to path containing the location id in JSON format
		let mut dotfile = File::create(path.with_file_name(DOTFILE_NAME))
			.await
			.map_err(|e| LocationError::DotfileWriteFailure(e, path.to_owned()))?;

		let data = DotSpacedrive {
			location_uuid: uuid,
			library_uuid: library.id,
		};

		let json_bytes = serde_json::to_vec(&data)
			.map_err(|e| LocationError::DotfileSerializeFailure(e, path.to_owned()))?;

		dotfile
			.write_all(&json_bytes)
			.await
			.map_err(|e| LocationError::DotfileWriteFailure(e, path.to_owned()))?;

		invalidate_query!(
			library,
			"locations.get": LibraryArgs<()>,
			LibraryArgs {
				library_id: library.id,
				arg: ()
			}
		);

		location
	};

	Ok(location_resource)
}

#[derive(Error, Debug)]
pub enum LocationError {
	#[error("Failed to create location (uuid {uuid:?})")]
	CreateFailure { uuid: Uuid },
	#[error("Failed to read location dotfile (path: {1:?})")]
	DotfileReadFailure(io::Error, PathBuf),
	#[error("Failed to serialize dotfile for location (at path: {1:?})")]
	DotfileSerializeFailure(serde_json::Error, PathBuf),
	#[error("Dotfile location is read only (at path: {0:?})")]
	ReadonlyDotFileLocationFailure(PathBuf),
	#[error("Failed to write dotfile (path: {1:?})")]
	DotfileWriteFailure(io::Error, PathBuf),
	#[error("Location not found (path: {0:?})")]
	PathNotFound(PathBuf),
	#[error("Location not found (uuid: {0})")]
	UuidNotFound(Uuid),
	#[error("Location not found (id: {0})")]
	IdNotFound(i32),
	#[error("Failed to open file from local os")]
	FileReadError(io::Error),
	#[error("Failed to read mounted volumes from local os")]
	VolumeReadError(String),
	#[error("Failed to connect to database (error: {0:?})")]
	IOError(io::Error),
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
}

impl From<LocationError> for rspc::Error {
	fn from(err: LocationError) -> Self {
		rspc::Error::new(ErrorCode::InternalServerError, err.to_string())
	}
}
