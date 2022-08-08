use p2p::Identity;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, RwLockWriteGuard};
use ts_rs::TS;
use uuid::Uuid;

/// NODE_STATE_CONFIG_NAME is the name of the file which stores the NodeState
pub const NODE_STATE_CONFIG_NAME: &str = "node_state.sdconfig";

/// ConfigMetadata is a part of node configuration that is loaded before the main configuration and contains information about the schema of the config.
/// This allows us to migrate breaking changes to the config format between Spacedrive releases.
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export)]
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
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct NodeConfig {
	#[serde(flatten)]
	pub metadata: ConfigMetadata,
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	/// the port this node uses for peer to peer communication. By default a random free port will be chosen each time the application is started.
	pub p2p_port: Option<u32>,
	/// The P2P identity public key
	pub p2p_cert: Vec<u8>,
	/// The P2P identity private key
	pub p2p_key: Vec<u8>,
	/// The address of the Spacetunnel discovery service being used.
	pub spacetunnel_addr: Option<String>,
}

#[derive(Error, Debug)]
pub enum NodeConfigError {
	#[error("error saving or loading the config from the filesystem")]
	IOError(#[from] io::Error),
	#[error("error serializing or deserializing the JSON in the config file")]
	JsonError(#[from] serde_json::Error),
	#[error("error migrating the config file")]
	MigrationError(String),
}

impl NodeConfig {
	fn default() -> Self {
		let identity = Identity::new().unwrap();
		let (p2p_cert, p2p_key) = identity.to_raw();
		NodeConfig {
			id: Uuid::new_v4(),
			name: match hostname::get() {
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
			p2p_cert,
			p2p_key,
			spacetunnel_addr: None,
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
				let (base_config, raw_config): (ConfigMetadata, Value) = {
					let mut file = File::open(&path)?;
					let raw_config: Value = serde_json::from_reader(BufReader::new(&mut file))?;
					(serde_json::from_value(raw_config.clone())?, raw_config)
				};
				Self::migrate_config(base_config.version, &path, raw_config).await?;

				let mut file = File::open(&path)?;
				let x = serde_json::from_reader(BufReader::new(&mut file))?;
				println!("{:?}", x);
				Ok(x)
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
	async fn migrate_config(
		current_version: Option<String>,
		config_path: &PathBuf,
		mut raw_config: Value,
	) -> Result<(), NodeConfigError> {
		match current_version.as_deref() {
			None => {
				Err(NodeConfigError::MigrationError(format!("Your Spacedrive config file stored at '{}' is missing the `version` field. If you just upgraded please delete the file and restart Spacedrive! Please note this upgrade will stop using your old 'library.db' as the folder structure has changed.", config_path.display())))
			}
            Some("0.1.0") => {
				let identity = Identity::new().expect("Error migrating config: unable to create P2P identity");

				if let Value::Object(obj) = &mut raw_config {
					let (cert, key) = identity.to_raw();
					obj.insert("p2p_cert".to_string(), serde_json::to_value(cert).unwrap());
					obj.insert("p2p_key".to_string(), serde_json::to_value(key).unwrap());
                }
				*raw_config.get_mut("version").unwrap() = Value::String("0.2.0".into());
				File::create(config_path)?.write_all(serde_json::to_string(&raw_config)?.as_bytes())?;
				println!("Migrated your config to version '0.2.0'");
				Ok(())
            },
            _ => Ok(()),
		}
	}
}
