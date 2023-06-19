use sd_p2p::Keypair;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use specta::Type;
use std::{
	path::{Path, PathBuf},
	sync::Arc,
};
use tokio::sync::{RwLock, RwLockWriteGuard};
use uuid::Uuid;

use crate::util::migrator::{Migrate, MigratorError};

/// NODE_STATE_CONFIG_NAME is the name of the file which stores the NodeState
pub const NODE_STATE_CONFIG_NAME: &str = "node_state.sdconfig";

/// NodeConfig is the configuration for a node. This is shared between all libraries and is stored in a JSON file on disk.
#[derive(Debug, Serialize, Deserialize, Clone)] // If you are adding `specta::Type` on this your probably about to leak the P2P private key
pub struct NodeConfig {
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	// the port this node uses for peer to peer communication. By default a random free port will be chosen each time the application is started.
	pub p2p_port: Option<u32>,
	/// The p2p identity keypair for this node. This is used to identify the node on the network.
	/// This keypair does effectively nothing except for provide libp2p with a stable peer_id.
	pub keypair: Keypair,
	// TODO: These will probs be replaced by your Spacedrive account in the near future.
	pub p2p_email: Option<String>,
	pub p2p_img_url: Option<String>,
}

// A version of [NodeConfig] that is safe to share with the frontend
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct SanitisedNodeConfig {
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: Uuid,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	// the port this node uses for peer to peer communication. By default a random free port will be chosen each time the application is started.
	pub p2p_port: Option<u32>,
	// TODO: These will probs be replaced by your Spacedrive account in the near future.
	pub p2p_email: Option<String>,
	pub p2p_img_url: Option<String>,
}

impl From<NodeConfig> for SanitisedNodeConfig {
	fn from(value: NodeConfig) -> Self {
		Self {
			id: value.id,
			name: value.name,
			p2p_port: value.p2p_port,
			p2p_email: value.p2p_email,
			p2p_img_url: value.p2p_img_url,
		}
	}
}

#[async_trait::async_trait]
impl Migrate for NodeConfig {
	const CURRENT_VERSION: u32 = 0;

	type Ctx = ();

	fn default(_path: PathBuf) -> Result<Self, MigratorError> {
		Ok(Self {
			id: Uuid::new_v4(),
			name: match hostname::get() {
				// SAFETY: This is just for display purposes so it doesn't matter if it's lossy
				Ok(hostname) => hostname.to_string_lossy().into_owned(),
				Err(err) => {
					eprintln!("Falling back to default node name as an error occurred getting your systems hostname: '{err}'");
					"my-spacedrive".into()
				}
			},
			p2p_port: None,
			keypair: Keypair::generate(),
			p2p_email: None,
			p2p_img_url: None,
		})
	}

	async fn migrate(
		from_version: u32,
		_config: &mut Map<String, Value>,
		_ctx: &Self::Ctx,
	) -> Result<(), MigratorError> {
		match from_version {
			0 => Ok(()),
			v => unreachable!("Missing migration for library version {}", v),
		}
	}
}

impl Default for NodeConfig {
	fn default() -> Self {
		NodeConfig {
			id: Uuid::new_v4(),
			name: match hostname::get() {
				// SAFETY: This is just for display purposes so it doesn't matter if it's lossy
				Ok(hostname) => hostname.to_string_lossy().into_owned(),
				Err(err) => {
					eprintln!("Falling back to default node name as an error occurred getting your systems hostname: '{err}'");
					"my-spacedrive".into()
				}
			},
			p2p_port: None,
			keypair: Keypair::generate(),
			p2p_email: None,
			p2p_img_url: None,
		}
	}
}

pub struct NodeConfigManager(RwLock<NodeConfig>, PathBuf);

impl NodeConfigManager {
	/// new will create a new NodeConfigManager with the given path to the config file.
	pub(crate) async fn new(data_path: PathBuf) -> Result<Arc<Self>, MigratorError> {
		Ok(Arc::new(Self(
			RwLock::new(NodeConfig::load_and_migrate(&Self::path(&data_path), &()).await?),
			data_path,
		)))
	}

	fn path(base_path: &Path) -> PathBuf {
		base_path.join(NODE_STATE_CONFIG_NAME)
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
	) -> Result<NodeConfig, MigratorError> {
		mutation_fn(self.0.write().await);
		let config = self.0.read().await;
		Self::save(&self.1, &config)?;
		Ok(config.clone())
	}

	/// save will write the configuration back to disk
	fn save(base_path: &Path, config: &NodeConfig) -> Result<(), MigratorError> {
		NodeConfig::save(config, &Self::path(base_path))?;
		Ok(())
	}
}
