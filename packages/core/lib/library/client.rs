use anyhow::Result;
use std::env;
// use sea_orm::EntityTrait;
use crate::{
    db::{
        connection::db,
        entity::client::{self, Platform},
    },
    state,
};
use sea_orm::ActiveModelTrait;
use sea_orm::Set;

pub async fn create() -> Result<()> {
    println!("Creating client...");
    let mut config = state::client::get();

    let db = db().await.expect("Could not connect to database");

    let hostname = match hostname::get() {
        Ok(hostname) => hostname.to_str().unwrap_or_default().to_owned(),
        Err(_) => "unknown".to_owned(),
    };

    let platform = match env::consts::OS {
        "windows" => Platform::Windows,
        "macos" => Platform::MacOS,
        "linux" => Platform::Linux,
        _ => Platform::Unknown,
    };

    let mut client = client::ActiveModel {
        name: Set(hostname.clone()),
        uuid: Set(config.client_id.clone()),
        platform: Set(platform),
        online: Set(true),
        ..Default::default()
    };

    client = client.save(db).await?;

    config.client_name = hostname;
    config.save();

    println!("Created client: {:?}", &client);

    Ok(())
}

// fn update_client_config(key: String, value: String) -> Result<&'static DotClientData> {
//     let mut client = client_config.get().unwrap();
//
//     let existing_value = client[key];
//
//     Ok(client)
// }
