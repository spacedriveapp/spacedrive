use anyhow::Result;
use config::{Config, File, FileFormat};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::sync::RwLock;

#[derive(Debug, Deserialize)]
pub struct ClientState {
    pub client_id: String,
    pub client_name: String,
    pub tcp_port: u32,

    #[serde(rename = "arr")]
    pub libraries: Vec<Library>,
}

#[derive(Debug, Deserialize)]
pub struct Library {
    pub library_id: String,
    pub data_folder_path: String,
}

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config::default());
}

pub fn get() -> Result<ClientState> {
    let rw_lock = CONFIG.read().unwrap();

    let client_state: ClientState = rw_lock.try_deserialize().unwrap();

    Ok(client_state)
}

pub fn make(path: &str) -> ClientState {
    let config_file_uri = format!("{}/config", path);

    let config = Config::builder()
        .add_source(File::new(&config_file_uri, FileFormat::Yaml))
        .set_default("client_id", "1")
        .unwrap_or_default()
        .build()
        .unwrap_or_default();

    let mut lock = CONFIG.write().unwrap();

    *lock = config;

    get().unwrap()
}
