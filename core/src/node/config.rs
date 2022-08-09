use rspc::Type;
use serde::{Deserialize, Serialize};
use std::{
	fs::File,
	io::{self, BufReader, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
	sync::Arc,
};
use thiserror::Error;
use tokio::sync::{RwLock, RwLockWriteGuard};

use uuid::Uuid;

/// NODE_STATE_CONFIG_NAME is the name of the file which stores the NodeState
pub const NODE_STATE_CONFIG_NAME: &str = "node_state.sdconfig";

/// ConfigMetadata is a part of node configuration that is loaded before the main configuration and contains information about the schema of the config.
/// This allows us to migrate breaking changes to the config format between Spacedrive releases.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct ConfigMetadata {
	/// version of Spacedrive. Determined from `CARGO_PKG_VERSION` environment variable.
	pub version: Option<String>,
}

impl Default for ConfigMetadata {
	fn default() -> Self {
		Self {
			version: Some(env!("CARGO_PKG_VERSION").into()),
		}
	}
}

/// NodeConfig is the configuration for a node. This is shared between all libraries and is stored in a JSON file on disk.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct NodeConfig {
	#[serde(flatten)]
	pub metadata: ConfigMetadata,
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	// the port this node uses for peer to peer communication. By default a random free port will be chosen each time the application is started.
	pub p2p_port: Option<u32>,
}

#[derive(Error, Debug)]
pub enum NodeConfigError {
	#[error("error saving or loading the config from the filesystem")]
	IO(#[from] io::Error),
	#[error("error serializing or deserializing the JSON in the config file")]
	Json(#[from] serde_json::Error),
	#[error("error migrating the config file")]
	Migration(String),
}

impl NodeConfig {
	fn default() -> Self {
		NodeConfig {
			id: Uuid::new_v4(),
			name: match hostname::get() {
				// SAFETY: This is just for display purposes so it doesn't matter if it's lossy
				Ok(hostname) => hostname.to_string_lossy().into_owned(),
				Err(err) => {
					eprintln!("Falling back to default node name as an error occurred getting your systems hostname: '{}'", err);
					"my-spacedrive".into()
				}
			},
			p2p_port: None,
			metadata: ConfigMetadata {
				version: Some(env!("CARGO_PKG_VERSION").into()),
			},
		}
	}
}

pub struct NodeConfigManager(RwLock<NodeConfig>, PathBuf);

impl NodeConfigManager {
	/// new will create a new NodeConfigManager with the given path to the config file.
	pub(crate) async fn new(data_path: PathBuf) -> Result<Arc<Self>, NodeConfigError> {
		Ok(Arc::new(Self(
			RwLock::new(Self::read(&data_path).await?),
			data_path,
		)))
	}

	/// get will return the current NodeConfig in a read only state.
	pub(crate) async fn get(&self) -> NodeConfig {
		self.0.read().await.clone()
	}

	/// data_directory returns the path to the directory storing the configuration data.
	pub(crate) fn data_directory(&self) -> PathBuf {
		self.1.clone()
	}

	/// write allows the user to update the configuration. This is done in a closure while a Mutex lock is held so that the user can't cause a race condition if the config were to be updated in multiple parts of the app at the same time.
	#[allow(unused)]
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
	async fn read(base_path: &PathBuf) -> Result<NodeConfig, NodeConfigError> {
		let path = Path::new(base_path).join(NODE_STATE_CONFIG_NAME);

		match path.exists() {
			true => {
				let mut file = File::open(&path)?;
				let base_config: ConfigMetadata =
					serde_json::from_reader(BufReader::new(&mut file))?;

				Self::migrate_config(base_config.version, path)?;

				file.seek(SeekFrom::Start(0))?;
				Ok(serde_json::from_reader(BufReader::new(&mut file))?)
			}
			false => {
				let config = NodeConfig::default();
				Self::save(base_path, &config).await?;
				Ok(config)
			}
		}
	}

	/// save will write the configuration back to disk
	async fn save(base_path: &PathBuf, config: &NodeConfig) -> Result<(), NodeConfigError> {
		let path = Path::new(base_path).join(NODE_STATE_CONFIG_NAME);
		File::create(path)?.write_all(serde_json::to_string(config)?.as_bytes())?;
		Ok(())
	}

	/// migrate_config is a function used to apply breaking changes to the config file.
	fn migrate_config(
		current_version: Option<String>,
		config_path: PathBuf,
	) -> Result<(), NodeConfigError> {
		match current_version {
			None => {
				Err(NodeConfigError::Migration(format!("Your Spacedrive config file stored at '{}' is missing the `version` field. If you just upgraded please delete the file and restart Spacedrive! Please note this upgrade will stop using your old 'library.db' as the folder structure has changed.", config_path.display())))
			}
			_ => Ok(()),
		}
	}
}
