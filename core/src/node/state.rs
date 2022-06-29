use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;
use tokio::io::AsyncReadExt;
use tokio::{
	fs,
	io::{AsyncWriteExt, BufReader},
};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, Default, TS)]
#[ts(export)]
pub struct NodeState {
	pub node_pub_id: String,
	pub node_id: i32,
	pub node_name: String,
	// config path is stored as struct can exist only in memory during startup and be written to disk later without supplying path
	pub data_path: Option<PathBuf>,
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
	pub library_path: PathBuf,
	pub offline: bool,
}

// global, thread-safe storage for node state
lazy_static! {
	static ref CONFIG: RwLock<Option<NodeState>> = RwLock::new(None);
}

pub fn get_nodestate() -> NodeState {
	if let Ok(guard) = CONFIG.read() {
		guard.clone().unwrap_or_default()
	} else {
		NodeState::default()
	}
}

impl NodeState {
	pub fn new(data_path: PathBuf, node_name: &str) -> Result<Self, ()> {
		let uuid = Uuid::new_v4().to_string();
		// create struct and assign defaults
		let config = Self {
			node_pub_id: uuid,
			data_path: Some(data_path),
			node_name: node_name.to_string(),
			..Default::default()
		};
		Ok(config)
	}

	pub async fn save(&self) {
		self.write_memory();
		// only write to disk if config path is set
		if let Some(ref data_path) = self.data_path {
			let config_path = data_path.join(NODE_STATE_CONFIG_NAME);
			let mut file = fs::File::create(config_path).await.unwrap();
			let json = serde_json::to_string(&self).unwrap();
			file.write_all(json.as_bytes()).await.unwrap();
		}
	}

	pub async fn read_disk(&mut self) -> Result<(), ()> {
		if let Some(ref data_path) = self.data_path {
			let config_path = data_path.join(NODE_STATE_CONFIG_NAME);
			// open the file and parse json
			if let Ok(file) = fs::File::open(config_path).await {
				let mut buf = vec![];
				let bytes = BufReader::new(file).read_to_end(&mut buf).await.unwrap();
				let data = serde_json::from_slice(&buf[..bytes]).unwrap();
				// assign to self
				*self = data;
			}
		}

		Ok(())
	}

	fn write_memory(&self) {
		let mut writeable = CONFIG.write().unwrap();
		*writeable = Some(self.clone());
	}

	pub fn get_current_library(&self) -> LibraryState {
		self.libraries
			.iter()
			.find(|lib| lib.library_uuid == self.current_library_uuid)
			.cloned()
			.unwrap_or_default()
	}

	pub fn get_current_library_db_path(&self) -> PathBuf {
		self.get_current_library().library_path.join("library.db")
	}
}
