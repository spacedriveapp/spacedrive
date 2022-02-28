use anyhow::Result;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, Write};
use std::sync::RwLock;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClientState {
    // client id is a uniquely generated UUID
    pub client_id: String,
    // client_name is the name of the device running the client
    pub client_name: String,
    // config path is stored as struct can exist only in memory during startup and be written to disk later without supplying path
    pub data_path: String,
    // the port this client uses to listen for incoming connections
    pub tcp_port: u32,
    // all the libraries loaded by this client
    pub libraries: Vec<LibraryState>,
    // used to quickly find the default library
    pub primary_library_id: String,
}

pub static CLIENT_STATE_CONFIG_NAME: &str = ".client_state";

impl Default for ClientState {
    fn default() -> Self {
        ClientState {
            client_id: "".to_string(),
            data_path: "".to_string(),
            client_name: "".to_string(),
            tcp_port: 0,
            libraries: vec![],
            primary_library_id: "".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LibraryState {
    pub library_id: String,
    pub library_path: String,
}

impl Default for LibraryState {
    fn default() -> Self {
        LibraryState {
            library_id: "".to_string(),
            library_path: "".to_string(),
        }
    }
}

// global, thread-safe storage for client state
lazy_static! {
    static ref CONFIG: RwLock<Option<ClientState>> = RwLock::new(None);
}

pub fn get() -> Result<ClientState> {
    let client_state = CONFIG.read().unwrap().as_ref().unwrap().clone();
    Ok(client_state)
}

impl ClientState {
    pub fn new(data_path: &str, client_name: &str) -> Result<Self> {
        // create struct and assign defaults
        let config = Self {
            data_path: data_path.to_string(),
            client_name: client_name.to_string(),
            ..Default::default()
        };
        Ok(config)
    }

    pub fn save(&self) {
        self.write_memory();
        // only write to disk if config path is set
        if !&self.data_path.is_empty() {
            let config_path = format!("{}/{}", &self.data_path, CLIENT_STATE_CONFIG_NAME);
            let mut file = fs::File::create(config_path).unwrap();
            let json = serde_json::to_string(&self).unwrap();
            file.write_all(json.as_bytes()).unwrap();
        }
    }

    pub fn read_disk(&mut self) -> Result<()> {
        let config_path = format!("{}/{}", &self.data_path, CLIENT_STATE_CONFIG_NAME);
        // open the file and parse json
        let file = fs::File::open(config_path)?;
        let reader = BufReader::new(file);
        let data = serde_json::from_reader(reader)?;

        println!("data:::: {:?}", data);
        // assign to self
        *self = data;
        Ok(())
    }

    fn write_memory(&self) {
        {
            let mut writeable = CONFIG.write().unwrap();
            *writeable = Some(self.clone());
        }
    }

    pub fn get_primary_library(&self) -> LibraryState {
        match self
            .libraries
            .iter()
            .find(|lib| lib.library_id == self.primary_library_id)
        {
            Some(lib) => lib.clone(),
            None => LibraryState::default(),
        }
    }

    pub fn get_current_library_db_path(&self) -> String {
        format!("{}/library.db", &self.get_primary_library().library_path)
    }
}
