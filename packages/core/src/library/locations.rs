use crate::{
    db,
    library::{volumes, volumes::Volume},
    prisma::{File, Location, LocationData},
    state::client,
};
use anyhow::Result;
use log::info;
use serde::{Deserialize, Serialize};
use std::{fs, io, io::Write};
use thiserror::Error;

#[derive(Serialize, Deserialize)]
pub struct DotSpaceDrive {
    pub location_uuid: String,
    pub library_uuid: String,
}

static DOTFILE_NAME: &str = ".spacedrive";

// checks to see if a location is:
// - accessible on from the local filesystem
// - already exists in the database
pub async fn check_location(path: &str) -> Result<DotSpaceDrive, LocationError> {
    let dotfile: DotSpaceDrive = match fs::File::open(format!("{}/{}", path.clone(), DOTFILE_NAME))
    {
        Ok(file) => serde_json::from_reader(file).unwrap(),
        Err(e) => return Err(LocationError::DotfileReadFailure(e)),
    };

    Ok(dotfile)
}

pub async fn get_location(location_id: i64) -> Result<LocationData, LocationError> {
    let db = db::get().await.map_err(|e| LocationError::DBError(e))?;

    // get location by location_id from db and include location_paths
    let location = match db
        .location()
        .find_first(vec![
            Location::files().some(vec![File::id().equals(location_id)])
        ])
        .exec()
        .await
    {
        Some(location) => location,
        None => return Err(LocationError::NotFound(location_id.to_string())),
    };

    info!("Retrieved location: {:?}", location);

    Ok(location)
}

pub async fn create_location(path: &str) -> Result<LocationData, LocationError> {
    let db = db::get().await.map_err(|e| LocationError::DBError(e))?;
    let config = client::get();

    // check if we have access to this location
    match fs::File::open(&path) {
        Ok(_) => info!("Path is valid, creating location for '{}'", &path),
        Err(e) => return Err(LocationError::FileReadError(e)),
    }
    // check if location already exists
    let location = match db
        .location()
        .find_first(vec![Location::path().equals(path.to_string())])
        .exec()
        .await
    {
        Some(location) => location,
        None => {
            info!(
                "Location does not exist, creating new location for '{}'",
                &path
            );
            let uuid = uuid::Uuid::new_v4();
            // create new location
            let create_location_params = {
                let volumes = match volumes::get() {
                    Ok(volumes) => volumes,
                    Err(e) => return Err(LocationError::VolumeReadError(e)),
                };
                info!("Loaded mounted volumes: {:?}", volumes);
                // find mount with matching path
                let volume = volumes
                    .into_iter()
                    .find(|mount| path.starts_with(&mount.mount_point));

                let volume_data = match volume {
                    Some(mount) => mount,
                    None => Volume::default(),
                };

                let mut create_location_params = vec![
                    Location::name().set(volume_data.name.to_string()),
                    Location::total_capacity().set(volume_data.total_space as i64),
                    Location::available_capacity().set(volume_data.available_space as i64),
                    Location::is_ejectable().set(false), // remove this
                    Location::is_removable().set(volume_data.is_removable),
                    Location::is_root_filesystem().set(false), // remove this
                    Location::is_online().set(true),
                ];
                create_location_params.extend(vec![
                    Location::path().set(path.to_string()),
                    // Location::library_id().set(library.id),
                ]);
                info!("Created new location: {:?}", location);
                create_location_params
            };

            let location = db
                .location()
                .create_one(create_location_params)
                .exec()
                .await;

            // write a file called .spacedrive to path containing the location id in JSON format
            let mut dotfile = match fs::File::create(format!("{}/{}", path.clone(), DOTFILE_NAME)) {
                Ok(file) => file,
                Err(e) => return Err(LocationError::DotfileWriteFailure(e, path.to_string())),
            };

            let data = DotSpaceDrive {
                location_uuid: uuid.to_string(),
                library_uuid: config.current_library_id,
            };

            let json = match serde_json::to_string(&data) {
                Ok(json) => json,
                Err(e) => return Err(LocationError::DotfileSerializeFailure(e, path.to_string())),
            };

            match dotfile.write_all(json.as_bytes()) {
                Ok(_) => (),
                Err(e) => return Err(LocationError::DotfileWriteFailure(e, path.to_string())),
            }

            location
        }
    };

    Ok(location)
}

#[derive(Error, Debug)]
pub enum LocationError {
    #[error("Failed to create location (uuid {uuid:?})")]
    CreateFailure { uuid: String },
    #[error("Failed to read location dotfile")]
    DotfileReadFailure(io::Error),
    #[error("Failed to serialize dotfile for location (at path: {0:?})")]
    DotfileSerializeFailure(serde_json::Error, String),
    #[error("Location not found (uuid: {0:?})")]
    DotfileWriteFailure(io::Error, String),
    #[error("Location not found (uuid: {0:?})")]
    NotFound(String),
    #[error("Failed to open file from local os")]
    FileReadError(io::Error),
    #[error("Failed to read mounted volumes from local os")]
    VolumeReadError(String),
    #[error("Failed to connect to database (error: {0:?})")]
    IOError(io::Error),
    #[error("Failed to connect to database (error: {0:?})")]
    DBError(String),
}
