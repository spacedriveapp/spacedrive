use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

use crate::{
    db, library,
    native::methods::get_mounts,
    prisma::{File, Location, LocationData},
};

#[derive(Serialize, Deserialize)]
struct DotSpaceDrive {
    location_id: i64,
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

    // get highest location id from db
    let next_location_id = match db.location().find_first(vec![]).exec().await {
        Some(location) => location.id + 1,
        None => 1,
    };

    println!("creating location: {}", path);

    // let file_name = Path::new(path)
    //     .file_name()
    //     .unwrap()
    //     .to_str()
    //     .unwrap()
    //     .to_string();

    let create_location_params = {
        // let library = library::loader::get().await?;
        let mounts = get_mounts();

        // find mount with matching path
        let mount = &mounts[0];
        //     .iter()
        //     .find(|mount| path.starts_with(mount.path.as_str()))
        // {
        //     Some(mount) => mount,
        //     None => {
        //         return Err(anyhow::anyhow!("{} is not a valid mount", path));
        //     }
        // };

        let mut create_location_params = vec![
            Location::name().set(mount.name.to_string()),
            Location::total_capacity().set(mount.total_capacity as i64),
            Location::available_capacity().set(mount.available_capacity as i64),
            Location::is_ejectable().set(mount.is_ejectable),
            Location::is_removable().set(mount.is_removable),
            Location::is_root_filesystem().set(mount.is_root_filesystem),
            Location::is_online().set(true),
        ];

        create_location_params.extend(vec![
            Location::id().set(next_location_id),
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
        location_id: next_location_id,
    };
    let json = serde_json::to_string(&data)?;

    dotfile.write_all(json.as_bytes())?;

    Ok(location)
}
