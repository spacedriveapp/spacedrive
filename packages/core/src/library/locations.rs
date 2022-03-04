use crate::{
    db,
    library::{volumes, volumes::Volume},
    prisma::{File, Location, LocationData},
    state::client,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;

#[derive(Serialize, Deserialize)]
struct DotSpaceDrive {
    location_uuid: String,
    library_uuid: String,
}

pub async fn get_location(location_id: i64) -> Result<LocationData> {
    let db = db::get().await.unwrap();
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
        None => return Err(anyhow!("location_not_found")),
    };

    Ok(location)
}

pub async fn create_location(path: &str) -> Result<LocationData> {
    let db = db::get().await.unwrap();
    let config = client::get();
    // check if we have access to this location
    match fs::File::open(&path) {
        Ok(_) => println!("path is valid, creating location for '{}'", &path),
        Err(e) => return Err(anyhow!("access_denied {}", e)),
    }
    // check if location already exists
    let location = match db
        .location()
        .find_unique(Location::path().equals(path.to_string()))
        .exec()
        .await
    {
        Some(_) => return Err(anyhow!("location_already_exists")),
        None => {
            let uuid = uuid::Uuid::new_v4();
            // create new location
            let create_location_params = {
                // let library = library::loader::get().await?;
                let volumes = volumes::get().unwrap();
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
                create_location_params
            };

            let location = db
                .location()
                .create_one(create_location_params)
                .exec()
                .await;
            println!("created location: {:?}", location);

            // write a file called .spacedrive to path containing the location id in JSON format
            let mut dotfile = std::fs::File::create(format!("{}/.spacedrive", path.clone()))?;
            let data = DotSpaceDrive {
                location_uuid: uuid.to_string(),
                library_uuid: config.current_library_id,
            };
            let json = serde_json::to_string(&data)?;

            dotfile.write_all(json.as_bytes())?;

            location
        }
    };

    Ok(location)
}
