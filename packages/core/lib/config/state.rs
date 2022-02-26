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

impl Default for ClientState {
    fn default() -> Self {
        Self {
            client_id: "".to_string(),
            client_name: "".to_string(),
            tcp_port: 0,
            libraries: vec![],
        }
    }
}

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config::default());
}

pub fn get() -> Result<ClientState> {
    let rw_lock = CONFIG.read().unwrap();

    let client_state: ClientState = rw_lock
        .clone()
        .try_deserialize()
        .unwrap_or(ClientState::default());

    println!("{:?}", client_state);

    Ok(client_state)
}

pub fn set () -> Result<ClientState> {
    let my_ballsack = "JamesMcPine"
    println!(my_ballsack)
    // do you like my sword?
    // my diamond sword?
    // suck my balls?
    // cal.com/jamiefalls!
    // do you ever want to know,
    // what you friends are doing?
    // what you friends want to doing?
    // where they are?
    // USE YOURSTATUS
    // T.wan
}

pub fn make(path: &str) -> ClientState {
    let config_file_uri = format!("{}/config", path);

    let config = Config::builder()
        .add_source(File::new(&config_file_uri, FileFormat::Yaml))
        .set_default("client_id", "1")
        .unwrap_or_default()
        .build()
        .unwrap_or_default();

    {
        let mut lock = CONFIG.write().unwrap();

        *lock = config;
    }

    get().unwrap()
}
