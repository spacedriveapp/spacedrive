use anyhow::Result;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufReader, Write};
use std::sync::RwLock;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, Default, TS)]
#[ts(export)]
pub struct NodeState {
  pub node_pub_id: String,
  pub node_id: i32,
  pub node_name: String,
  // config path is stored as struct can exist only in memory during startup and be written to disk later without supplying path
  pub data_path: String,
  // the port this node uses to listen for incoming connections
  pub tcp_port: u32,
  // all the libraries loaded by this node
  pub libraries: Vec<LibraryState>,
  // used to quickly find the default library
  pub current_library_uuid: String,
}

pub static NODE_STATE_CONFIG_NAME: &str = "node_state.json";

#[derive(Debug, Serialize, Deserialize, Clone, Default, TS)]
#[ts(export)]
pub struct LibraryState {
  pub library_uuid: String,
  pub library_id: i32,
  pub library_path: String,
  pub offline: bool,
}

// global, thread-safe storage for node state
lazy_static! {
  static ref CONFIG: RwLock<Option<NodeState>> = RwLock::new(None);
}

pub fn get() -> NodeState {
  match CONFIG.read() {
    Ok(guard) => guard.clone().unwrap_or(NodeState::default()),
    Err(_) => return NodeState::default(),
  }
}

impl NodeState {
  pub fn new(data_path: &str, node_name: &str) -> Result<Self> {
    let uuid = Uuid::new_v4().to_string();
    // create struct and assign defaults
    let config = Self {
      node_pub_id: uuid,
      data_path: data_path.to_string(),
      node_name: node_name.to_string(),
      ..Default::default()
    };
    Ok(config)
  }

  pub fn save(&self) {
    self.write_memory();
    // only write to disk if config path is set
    if !&self.data_path.is_empty() {
      let config_path = format!("{}/{}", &self.data_path, NODE_STATE_CONFIG_NAME);
      let mut file = fs::File::create(config_path).unwrap();
      let json = serde_json::to_string(&self).unwrap();
      file.write_all(json.as_bytes()).unwrap();
    }
  }

  pub fn read_disk(&mut self) -> Result<()> {
    let config_path = format!("{}/{}", &self.data_path, NODE_STATE_CONFIG_NAME);
    // open the file and parse json
    let file = fs::File::open(config_path)?;
    let reader = BufReader::new(file);
    let data = serde_json::from_reader(reader)?;
    // assign to self
    *self = data;
    Ok(())
  }

  fn write_memory(&self) {
    let mut writeable = CONFIG.write().unwrap();
    *writeable = Some(self.clone());
  }

  pub fn get_current_library(&self) -> LibraryState {
    match self
      .libraries
      .iter()
      .find(|lib| lib.library_uuid == self.current_library_uuid)
    {
      Some(lib) => lib.clone(),
      None => LibraryState::default(),
    }
  }

  pub fn get_current_library_db_path(&self) -> String {
    format!("{}/library.db", &self.get_current_library().library_path)
  }
}
