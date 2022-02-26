use std::io::Write;
use std::{collections::HashMap, env};

use anyhow::Result;
use once_cell::sync::OnceCell;
use sea_orm::EntityTrait;
use sea_orm::Set;

use sea_orm::{ActiveModelTrait, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::{
    db::{
        connection::db_instance,
        entity::client::{self, Platform},
    },
    get_core_config,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub enum DotClientKey {
    ClientId { client_id: String },
    TCPPort { tcp_port: u64 },
}

// // client config file struct
// #[derive(Serialize, Deserialize, Debug)]
// pub struct DotClientData {
//     pub client_id: u32,
//     pub client_name: Option<String>,
//     pub tcp_port: Option<u32>,
// }

// in memory storage for config file
pub static CLIENT_CONFIG: OnceCell<HashMap<DotClientKey, String>> = OnceCell::new();

fn get_client_config() -> Result<&'static HashMap<DotClientKey, String>> {
    let client = CLIENT_CONFIG.get().unwrap();
    Ok(client)
}

// method to update client config
pub fn update_client_config(key: DotClientKey, json_value: String) -> Result<()> {
    let core_config = get_core_config();
    // clone existing config from memory
    let mut config = CLIENT_CONFIG.get().unwrap().clone();
    // insert new value
    config.insert(key, json_value);
    // set in memory
    CLIENT_CONFIG.set(config).unwrap();
    // convert to json
    let json = serde_json::to_string(&config)?;
    // create fresh dot file
    let mut dotfile =
        std::fs::File::create(format!("{}/.client_data", core_config.data_dir.display()))?;
    // write json to file
    dotfile.write_all(json.as_bytes())?;

    Ok(())
}

pub async fn init_client() -> Result<()> {
    let config = get_core_config();

    // read .client_data file from config.data_dir using serde to sterilize into DotClientData
    let client_data_path = format!("{}/.client_data", config.data_dir.display());
    let client_data_file = std::fs::File::open(&client_data_path);

    match client_data_file {
        Ok(file) => {
            let client_data: HashMap<DotClientKey, String> = serde_json::from_reader(file).unwrap();
            CLIENT_CONFIG.set(client_data);

            println!("loaded existing client: {:?}", CLIENT_CONFIG.get().unwrap());
        }
        Err(_) => {
            let client_data = create_client().await?;

            println!("created new client {:?}", CLIENT_CONFIG.get().unwrap());
        }
    };

    Ok(())
}

pub async fn create_client() -> Result<HashMap<DotClientKey, String>> {
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

    let json = serde_json::to_string(&data)?;

    dotfile.write_all(json.as_bytes())?;

    Ok(data)
}

// fn update_client_config(key: String, value: String) -> Result<&'static DotClientData> {
//     let mut client = client_config.get().unwrap();
//
//     let existing_value = client[key];
//
//     Ok(client)
// }
