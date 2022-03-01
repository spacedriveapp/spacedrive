use anyhow::{anyhow, Result};
use chrono::Utc;
use sea_orm::ColumnTrait;
use sea_orm::Set;
use sea_orm::{ActiveModelTrait, QueryOrder};
use sea_orm::{EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::io::Write;
use std::path::Path;

use crate::{
    db::{
        connection::db,
        entity::{file, locations},
    },
    library,
    native::methods::get_mounts,
};

#[derive(Serialize, Deserialize)]
struct DotSpaceDrive {
    location_id: u32,
}

pub async fn get_location(location_id: u32) -> Result<locations::Model> {
    let db = db().await.unwrap();

    // get location by location_id from db and include location_paths
    let location = match locations::Entity::find()
        .filter(file::Column::Id.eq(location_id))
        .one(db)
        .await
    {
        Ok(location) => location,
        Err(_) => return Err(anyhow!("location_not_found")),
    };

    Ok(location.unwrap())
}

pub async fn create_location(path: &str) -> Result<()> {
    let db = db().await.unwrap();

    let library = library::loader::get().await?;
    let mounts = get_mounts();

    // find mount with matching path
    let mount = match mounts.iter().find(|mount| mount.path.as_str() == path) {
        Some(mount) => mount,
        None => {
            return Err(anyhow::anyhow!("{} is not a valid mount", path));
        }
    };

    // get highest location id from db
    let next_location_id = match locations::Entity::find()
        .order_by_desc(locations::Column::Id)
        .one(db)
        .await
    {
        Ok(location) => location.map_or(1, |location| location.id + 1),
        Err(_) => 1,
    };

    let location = locations::ActiveModel {
        id: Set(next_location_id),
        client_id: Set(1),
        name: Set(mount.name.to_owned()),
        path: Set(Path::new(path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()),
        total_capacity: Set(mount.total_capacity.try_into().unwrap()),
        available_capacity: Set(mount.available_capacity.try_into().unwrap()),
        is_ejectable: Set(mount.is_ejectable),
        is_removable: Set(mount.is_removable),
        library_id: Set(library.id),
        date_created: Set(Some(Utc::now().naive_utc())),
        last_indexed: Set(Some(Utc::now().naive_utc())),
        is_root_filesystem: Set(mount.is_root_filesystem),
        is_online: Set(true),
        ..Default::default()
    };
    location.save(db).await?;

    // write a file called .spacedrive to path containing the location id in JSON format
    let mut dotfile = std::fs::File::create(format!("{}/.spacedrive", path))?;
    let data = DotSpaceDrive {
        location_id: next_location_id,
    };
    let json = serde_json::to_string(&data)?;

    dotfile.write_all(json.as_bytes())?;

    Ok(())
}
