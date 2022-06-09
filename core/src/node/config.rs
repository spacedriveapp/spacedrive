use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, RwLockWriteGuard};
use ts_rs::TS;
use uuid::Uuid;

/// NODE_STATE_CONFIG_NAME is the name of the file which stores the NodeState
pub const NODE_STATE_CONFIG_NAME: &str = "node_state.json";

/// NodeConfig is the configuration for a node. This is shared between all libraries and is stored in a JSON file on disk.
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct NodeConfig {
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

#[derive(Error, Debug)]
pub enum NodeConfigError {
	#[error("error saving or loading the config from the filesystem")]
	IOError(io::Error),
	#[error("error serializing or deserializing the JSON in the config file")]
	JsonError(serde_json::Error),
}

impl NodeConfig {
	fn default(data_path: String) -> Self {
		// TODO: Make these initialise to good values
		NodeConfig {
			node_pub_id: Uuid::new_v4().to_string(),
			node_id: 0,
			node_name: "diamond-mastering-space-dragon".into(), // TODO: Get from OS hostname or generate random
			data_path,
			tcp_port: 0,
			libraries: vec![],
			current_library_uuid: "".into(),
		}
	}
}

/// LibraryState stores information about a library that the user has.
#[derive(Debug, Serialize, Deserialize, Clone, Default, TS)]
#[ts(export)]
pub struct LibraryState {
	pub library_uuid: String,
	pub library_id: i32,
	pub library_path: String,
	pub offline: bool,
}

pub struct NodeConfigManager(RwLock<NodeConfig>, String);

impl NodeConfigManager {
	/// new will create a new NodeConfigManager with the given path to the config file.
	pub(crate) async fn new(data_path: String) -> Result<Arc<Self>, NodeConfigError> {
		Ok(Arc::new(Self(
			RwLock::new(Self::read(&data_path).await?),
			data_path,
		)))
	}

	/// get will return the current NodeConfig in a read only state.
	pub(crate) async fn get(&self) -> NodeConfig {
		self.0.read().await.clone()
	}

	// /// TODO
	// pub(crate) async fn get_library(&self, library_id: String) -> Option<LibraryState> {
	// 	self.0
	// 		.read()
	// 		.await
	// 		.libraries
	// 		.iter()
	// 		.find(|lib| lib.library_uuid == library_id)
	// 		.map(|lib| lib.clone())
	// }

	/// TODO: Move this function onto the `LibraryContext` when adding multi-library support.
	pub(crate) async fn get_current_library(&self) -> LibraryState {
		let config = self.0.read().await;

		config
			.libraries
			.iter()
			.find(|lib| lib.library_uuid == config.current_library_uuid)
			.map(|lib| lib.clone())
			.unwrap_or(LibraryState::default())
	}

	/// write allows the user to update the configuration. This is done in a closure while a Mutex lock is held so that the user can't cause a race condition if the config were to be updated in multiple parts of the app at the same time.
	pub(crate) async fn write<F: FnOnce(RwLockWriteGuard<NodeConfig>)>(
		&self,
		mutation_fn: F,
	) -> Result<NodeConfig, NodeConfigError> {
		mutation_fn(self.0.write().await);
		let config = self.0.read().await;
		Self::save(&self.1, &config).await?;
		Ok(config.clone())
	}

	/// read will read the configuration from disk and return it.
	async fn read(base_path: &str) -> Result<NodeConfig, NodeConfigError> {
		let path = Path::new(base_path).join(NODE_STATE_CONFIG_NAME);

		match path.exists() {
			true => {
				let reader = BufReader::new(File::open(path).map_err(NodeConfigError::IOError)?);
				Ok(serde_json::from_reader(reader).map_err(NodeConfigError::JsonError)?)
			}
			false => {
				let config = NodeConfig::default(base_path.into());
				Self::save(base_path, &config).await?;
				Ok(config)
			}
		}
	}

	/// save will write the configuration back to disk
	async fn save(base_path: &str, config: &NodeConfig) -> Result<(), NodeConfigError> {
		let path = Path::new(base_path).join(NODE_STATE_CONFIG_NAME);
		File::create(path)
			.map_err(NodeConfigError::IOError)?
			.write_all(
				serde_json::to_string(config)
					.map_err(NodeConfigError::JsonError)?
					.as_bytes(),
			)
			.map_err(NodeConfigError::IOError)?;
		Ok(())
	}
}
