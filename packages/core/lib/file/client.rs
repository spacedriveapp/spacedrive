use std::env;
use std::io::Write;

use anyhow::Result;
use once_cell::sync::OnceCell;
use sea_orm::{ActiveModelTrait, QueryOrder};
use sea_orm::EntityTrait;
use sea_orm::Set;
use serde::{Deserialize, Serialize};

use crate::{
    db::{
        connection::db_instance,
        entity::client::{self, Platform},
    },
    get_core_config,
};

// client config file struct
#[derive(Serialize, Deserialize, Debug)]
pub struct DotClientData {
    pub client_id: u32,
    pub client_name: Option<String>,
    pub tcp_port: Option<u32>
}

// in memory storage for config file
pub static client_config: OnceCell<DotClientData> = OnceCell::new();

pub async fn init_client() -> Result<()> {
    let config = get_core_config();

    // read .client_data file from config.data_dir using serde to sterilize into DotClientData
    let client_data_path = format!("{}/.client_data", config.data_dir.display());
    let client_data_file = std::fs::File::open(&client_data_path);

    match client_data_file {
        Ok(file) => {
            let client_data: DotClientData = serde_json::from_reader(file).unwrap();
            client_config.set(client_data);
            println!("loaded existing client: {:?}", client_config.get().unwrap());
        }
        Err(_) => {
            let client_data = create_client().await?;
            client_config.set(client_data);
            println!("created new client {:?}", client_config.get().unwrap());
        }
    };

    Ok(())
}

pub async fn create_client() -> Result<DotClientData> {
    let db = db_instance().await.unwrap();
    let config = get_core_config();

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

    let client = client::ActiveModel {
        // id: Set(next_client_id),
        name: Set(hostname),
        platform: Set(platform),
        online: Set(true),
        ..Default::default()
    };

    client.save(db).await.map_err(|e| {
        println!("error saving client: {:?}", e);
        e
    })?;

    // write a file called .spacedrive to path containing the location id in JSON format
    let mut dotfile = std::fs::File::create(format!("{}/.client_data", config.data_dir.display()))?;

    let data = DotClientData {
        client_id: next_client_id,
        tcp_port: None,
        client_name: None
    };

    let json = serde_json::to_string(&data)?;

    dotfile.write_all(json.as_bytes())?;

    Ok(data)
}

fn get_client_config() -> Result<&'static DotClientData> {
    let client = client_config.get().unwrap();
    Ok(client)
}

// fn update_client_config(key: String, value: String) -> Result<&'static DotClientData> {
//     let mut client = client_config.get().unwrap();
//
//     let existing_value = client[key];
//
//     Ok(client)
// }

