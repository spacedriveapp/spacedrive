use crate::{
	file::{cas::FileIdentifierJob, indexer::IndexerJob},
	node::{get_nodestate, LibraryNode},
	prisma::{file_path, location},
	ClientQuery, CoreContext, CoreEvent,
};

use log::info;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::{
	fs::{metadata, File},
	io,
};
use ts_rs::TS;
use uuid::Uuid;

use super::SysError;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LocationResource {
	pub id: i32,
	pub name: Option<String>,
	pub path: Option<String>,
	pub total_capacity: Option<i32>,
	pub available_capacity: Option<i32>,
	pub is_removable: Option<bool>,
	pub node: Option<LibraryNode>,
	pub is_online: bool,
	#[ts(type = "string")]
	pub date_created: chrono::DateTime<chrono::Utc>,
}

impl From<location::Data> for LocationResource {
	fn from(data: location::Data) -> Self {
		LocationResource {
			id: data.id,
			name: data.name,
			path: data.local_path,
			total_capacity: data.total_capacity,
			available_capacity: data.available_capacity,
			is_removable: data.is_removable,
			node: data.node.unwrap_or(None).map(Into::into),
			is_online: data.is_online,
			date_created: data.date_created.into(),
		}
	}
}

#[derive(Serialize, Deserialize, Default)]
pub struct DotSpacedrive {
	pub location_uuid: Uuid,
	pub library_uuid: String,
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

pub async fn get_location(
	ctx: &CoreContext,
	location_id: i32,
) -> Result<LocationResource, SysError> {
	// get location by location_id from db and include location_paths
	ctx.database
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await?
		.map(Into::into)
		.ok_or_else(|| LocationError::IdNotFound(location_id).into())
}

pub fn scan_location(ctx: &CoreContext, location_id: i32, path: impl AsRef<Path>) {
	let path_buf = path.as_ref().to_path_buf();
	ctx.spawn_job(Box::new(IndexerJob {
		path: path_buf.clone(),
	}));
	ctx.queue_job(Box::new(FileIdentifierJob {
		location_id,
		path: path_buf,
	}));
	// TODO: make a way to stop jobs so this can be canceled without rebooting app
	// ctx.queue_job(Box::new(ThumbnailJob {
	// 	location_id,
	// 	path: "".to_string(),
	// 	background: false,
	// }));
}

pub async fn new_location_and_scan(
	ctx: &CoreContext,
	path: impl AsRef<Path> + Debug,
) -> Result<LocationResource, SysError> {
	let location = create_location(ctx, &path).await?;

	scan_location(ctx, location.id, path);

	Ok(location)
}

pub async fn get_locations(ctx: &CoreContext) -> Result<Vec<LocationResource>, SysError> {
	let locations = ctx
		.database
		.location()
		.find_many(vec![])
		.with(location::node::fetch())
		.exec()
		.await?;

	// turn locations into LocationResource
	Ok(locations.into_iter().map(LocationResource::from).collect())
}

pub async fn create_location(
	ctx: &CoreContext,
	path: impl AsRef<Path> + Debug,
) -> Result<LocationResource, SysError> {
	let path = path.as_ref();

	// check if we have access to this location
	if !path.exists() {
		return Err(LocationError::PathNotFound(path.to_owned()).into());
	}

	if metadata(path)
		.await
		.map_err(|e| LocationError::DotfileReadFailure(e, path.to_owned()))?
		.permissions()
		.readonly()
	{
		return Err(LocationError::ReadonlyDotFileLocationFailure(path.to_owned()).into());
	}

	let path_string = path.to_string_lossy().to_string();

	// check if location already exists
	let location_resource = if let Some(location) = ctx
		.database
		.location()
		.find_first(vec![location::local_path::equals(Some(
			path_string.clone(),
		))])
		.exec()
		.await?
	{
		location.into()
	} else {
		info!(
			"Location does not exist, creating new location for '{:#?}'",
			path
		);
		let uuid = Uuid::new_v4();

		let config = get_nodestate();

		let location = ctx
			.database
			.location()
			.create(
				location::pub_id::set(uuid.to_string()),
				vec![
					location::name::set(Some(
						path.file_name().unwrap().to_string_lossy().to_string(),
					)),
					location::is_online::set(true),
					location::local_path::set(Some(path_string)),
					location::node_id::set(Some(config.node_id)),
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
			library_uuid: config.current_library_uuid,
		};

		let json_bytes = serde_json::to_vec(&data)
			.map_err(|e| LocationError::DotfileSerializeFailure(e, path.to_owned()))?;

		dotfile
			.write_all(&json_bytes)
			.await
			.map_err(|e| LocationError::DotfileWriteFailure(e, path.to_owned()))?;

		ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::SysGetLocations))
			.await;

		location.into()
	};

	Ok(location_resource)
}

pub async fn delete_location(ctx: &CoreContext, location_id: i32) -> Result<(), SysError> {
	ctx.database
		.file_path()
		.find_many(vec![file_path::location_id::equals(Some(location_id))])
		.delete()
		.exec()
		.await?;

	ctx.database
		.location()
		.find_unique(location::id::equals(location_id))
		.delete()
		.exec()
		.await?;

	ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::SysGetLocations))
		.await;

	info!("Location {} deleted", location_id);

	Ok(())
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
}
