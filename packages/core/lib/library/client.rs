use std::{collections::HashMap, env};

use anyhow::Result;

use sea_orm::EntityTrait;
use sea_orm::Set;

use sea_orm::{ActiveModelTrait, QueryOrder};

use crate::{
    db::{
        connection::db,
        entity::client::{self, Platform},
    },
    state,
};

pub async fn create() -> Result<()> {
    let db = db().await.unwrap();
    let config = state::client::get().unwrap();

    // get highest location id from db
    let next_client_id = match client::Entity::find()
        .order_by_desc(client::Column::Id)
        .one(db)
        .await
    {
        Ok(client) => client.map_or(1, |client| client.id + 1),
        Err(_) => 1,
    };

    let hostname = match hostname::get() {
        Ok(hostname) => hostname.to_str().unwrap().to_owned(),
        Err(_) => "unknown".to_owned(),
    };

    let platform = match env::consts::OS {
        "windows" => Platform::Windows,
        "macos" => Platform::MacOS,
        "linux" => Platform::Linux,
        _ => Platform::Unknown,
    };

    let mut client = client::ActiveModel {
        // id: Set(next_client_id),
        name: Set(hostname),
        platform: Set(platform),
        online: Set(true),
        ..Default::default()
    };

    client = client.save(db).await.map_err(|e| {
        println!("error saving client: {:?}", e);
        e
    })?;

    Ok(())
}

// fn update_client_config(key: String, value: String) -> Result<&'static DotClientData> {
//     let mut client = client_config.get().unwrap();
//
//     let existing_value = client[key];
//
//     Ok(client)
// }
