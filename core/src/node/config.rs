use crate::{
	api::{notifications::Notification, BackendFeature},
	/*object::media::old_thumbnail::preferences::ThumbnailerPreferences,*/
	util::version_manager::{Kind, ManagedVersion, VersionManager, VersionManagerError},
};

use sd_cloud_schema::devices::DeviceOS;
use sd_core_sync::DevicePubId;
use sd_p2p::Identity;
use sd_utils::error::FileIOError;

use std::{
	collections::HashSet,
	path::{Path, PathBuf},
	sync::Arc,
};

use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use serde_repr::{Deserialize_repr, Serialize_repr};
use specta::Type;
use thiserror::Error;
use tokio::{
	fs,
	sync::{watch, RwLock},
};
use tracing::error;
use uuid::Uuid;

use super::HardwareModel;

/// NODE_STATE_CONFIG_NAME is the name of the file which stores the NodeState
pub const NODE_STATE_CONFIG_NAME: &str = "node_state.sdconfig";

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Type)]
pub enum P2PDiscoveryState {
	#[default]
	Everyone,
	ContactsOnly,
	Disabled,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum Port {
	#[default]
	Random,
	Discrete(u16),
}

impl Port {
	pub fn get(&self) -> u16 {
		if is_in_docker() {
			return 7373;
		}

		match self {
			Port::Random => 0,
			Port::Discrete(port) => *port,
		}
	}

	pub fn is_default(&self) -> bool {
		matches!(self, Port::Random)
	}
}

pub fn is_in_docker() -> bool {
	std::env::var("SD_DOCKER").as_deref() == Ok("true")
}

fn skip_if_false(value: &bool) -> bool {
	!*value
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NodeConfigP2P {
	#[serde(default)]
	pub discovery: P2PDiscoveryState,
	#[serde(default, skip_serializing_if = "Port::is_default")]
	pub port: Port,
	#[serde(default, skip_serializing_if = "skip_if_false")]
	pub disabled: bool,
	#[serde(default, skip_serializing_if = "skip_if_false")]
	pub disable_ipv6: bool,
	#[serde(default, skip_serializing_if = "skip_if_false")]
	pub disable_relay: bool,
	#[serde(default, skip_serializing_if = "skip_if_false")]
	pub enable_remote_access: bool,
	/// A list of peer addresses to try and manually connect to, instead of relying on discovery.
	///
	/// All of these are valid values:
	///  - `localhost`
	///  - `spacedrive.com` or `spacedrive.com:3000`
	///  - `127.0.0.1` or `127.0.0.1:300`
	///  - `[::1]` or `[::1]:3000`
	///
	/// which is why we use `String` not `SocketAddr`
	#[serde(default)]
	pub manual_peers: HashSet<String>,
}

impl Default for NodeConfigP2P {
	fn default() -> Self {
		Self {
			discovery: P2PDiscoveryState::Everyone,
			port: Port::Random,
			disabled: true,
			disable_ipv6: true,
			disable_relay: true,
			enable_remote_access: false,
			manual_peers: Default::default(),
		}
	}
}

/// NodeConfig is the configuration for a node.
/// This is shared between all libraries and is stored in a JSON file on disk.
#[derive(Debug, Clone, Serialize, Deserialize)] // If you are adding `specta::Type` on this your probably about to leak the P2P private key
pub struct NodeConfig {
	/// id is a unique identifier for the current node. Each node has a public identifier (this one) and is given a local id for each library (done within the library code).
	pub id: DevicePubId,
	/// name is the display name of the current node. This is set by the user and is shown in the UI. // TODO: Length validation so it can fit in DNS record
	pub name: String,
	/// core level notifications
	#[serde(default)]
	pub notifications: Vec<Notification>,
	/// The p2p identity keypair for this node. This is used to identify the node on the network.
	/// This keypair does effectively nothing except for provide libp2p with a stable peer_id.
	#[serde(with = "identity_serde")]
	// TODO(@fogodev): remove these from here, we must not store secret keys in plaintext...
	// Put then on secret storage when we have a keyring compatible with all our supported platforms
	pub identity: Identity,
	/// P2P config
	#[serde(default)]
	pub p2p: NodeConfigP2P,
	/// Feature flags enabled on the node
	#[serde(default)]
	pub features: Vec<BackendFeature>,
	/// The aggregation of many different preferences for the node
	pub preferences: NodePreferences,
	/// Operating System of the node
	pub os: DeviceOS,
	/// Hardware model of the node
	pub hardware_model: HardwareModel,

	version: NodeConfigVersion,
}

mod identity_serde {
	use sd_p2p::Identity;
	use serde::{Deserialize, Deserializer, Serialize, Serializer};

	pub fn serialize<S>(identity: &Identity, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		to_string(identity).serialize(serializer)
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Identity, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Identity::from_bytes(&base91::slice_decode(s.as_bytes())).map_err(serde::de::Error::custom)
	}

	pub fn to_string(identity: &Identity) -> String {
		String::from_utf8_lossy(&base91::slice_encode(&identity.to_bytes())).to_string()
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Type)]
pub struct NodePreferences {
	// pub thumbnailer: ThumbnailerPreferences,
	// TODO(fogodev): introduce preferences to choose how many worker the task system should have
}

#[derive(
	IntEnum, Debug, Clone, Copy, Eq, PartialEq, strum::Display, Serialize_repr, Deserialize_repr,
)]
#[repr(u64)]
pub enum NodeConfigVersion {
	V0 = 0,
	V1 = 1,
	V2 = 2,
	V3 = 3,
	V4 = 4,
	V5 = 5,
}

impl ManagedVersion<NodeConfigVersion> for NodeConfig {
	const LATEST_VERSION: NodeConfigVersion = NodeConfigVersion::V5;
	const KIND: Kind = Kind::Json("version");
	type MigrationError = NodeConfigError;

	fn from_latest_version() -> Option<Self> {
		#[cfg(not(any(target_os = "ios", target_os = "android")))]
		let mut name = whoami::devicename();

		#[cfg(target_os = "ios")]
		let mut name = "iOS Device".to_string();

		#[cfg(target_os = "android")]
		let mut name = "Android Device".to_string();

		name.truncate(255);

		let os = DeviceOS::from_env();
		let hardware_model = HardwareModel::try_get().unwrap_or_else(|e| {
			error!(?e, "Failed to get hardware model");
			HardwareModel::Other
		});

		Some(Self {
			id: Uuid::now_v7().into(),
			name,
			identity: Identity::default(),
			p2p: NodeConfigP2P::default(),
			version: Self::LATEST_VERSION,
			features: vec![],
			notifications: vec![],
			preferences: NodePreferences::default(),
			os,
			hardware_model,
		})
	}
}

impl NodeConfig {
	pub async fn load(path: impl AsRef<Path>) -> Result<Self, NodeConfigError> {
		let path = path.as_ref();
		VersionManager::<Self, NodeConfigVersion>::migrate_and_load(
			path,
			|current, next| async move {
				match (current, next) {
					(NodeConfigVersion::V0, NodeConfigVersion::V1) => {
						let mut config: Map<String, Value> =
							serde_json::from_slice(&fs::read(path).await.map_err(|e| {
								FileIOError::from((
									path,
									e,
									"Failed to read node config file for migration",
								))
							})?)
							.map_err(VersionManagerError::SerdeJson)?;

						// All were never hooked up to the UI
						config.remove("p2p_email");
						config.remove("p2p_img_url");
						config.remove("p2p_port");

						// In a recent PR I screwed up Serde `default` so P2P was disabled by default, prior it was always enabled.
						// Given the config for it is behind a feature flag (so no one would have changed it) this fixes the default.
						if let Some(Value::Object(obj)) = config.get_mut("p2p") {
							obj.insert("enabled".into(), Value::Bool(true));
						}

						fs::write(
							path,
							serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?,
						)
						.await
						.map_err(|e| FileIOError::from((path, e)))?;
					}

					(NodeConfigVersion::V1, NodeConfigVersion::V2) => {
						let mut config: Map<String, Value> =
							serde_json::from_slice(&fs::read(path).await.map_err(|e| {
								FileIOError::from((
									path,
									e,
									"Failed to read node config file for migration",
								))
							})?)
							.map_err(VersionManagerError::SerdeJson)?;

						config.insert(
							String::from("preferences"),
							json!(NodePreferences::default()),
						);

						let a =
							serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?;

						fs::write(path, a)
							.await
							.map_err(|e| FileIOError::from((path, e)))?;
					}

					(NodeConfigVersion::V2, NodeConfigVersion::V3) => {
						let mut config: Map<String, Value> =
							serde_json::from_slice(&fs::read(path).await.map_err(|e| {
								FileIOError::from((
									path,
									e,
									"Failed to read node config file for migration",
								))
							})?)
							.map_err(VersionManagerError::SerdeJson)?;

						config.remove("keypair");
						config.remove("p2p");

						config.insert(
							String::from("identity"),
							json!(identity_serde::to_string(&Default::default())),
						);

						let a =
							serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?;

						fs::write(path, a)
							.await
							.map_err(|e| FileIOError::from((path, e)))?;
					}

					(NodeConfigVersion::V3, NodeConfigVersion::V4) => {
						let mut config: Map<String, Value> =
							serde_json::from_slice(&fs::read(path).await.map_err(|e| {
								FileIOError::from((
									path,
									e,
									"Failed to read node config file for migration",
								))
							})?)
							.map_err(VersionManagerError::SerdeJson)?;

						config.remove("id");
						config.insert(
							String::from("id"),
							serde_json::to_value(Uuid::now_v7())
								.map_err(VersionManagerError::SerdeJson)?,
						);

						config.remove("name");

						#[cfg(not(any(target_os = "ios", target_os = "android")))]
						config.insert(
							String::from("name"),
							serde_json::to_value(whoami::devicename())
								.map_err(VersionManagerError::SerdeJson)?,
						);

						#[cfg(target_os = "ios")]
						config.insert(
							String::from("name"),
							serde_json::to_value("iOS Device")
								.map_err(VersionManagerError::SerdeJson)?,
						);

						#[cfg(target_os = "android")]
						config.insert(
							String::from("name"),
							serde_json::to_value("Android Device")
								.map_err(VersionManagerError::SerdeJson)?,
						);

						config.insert(
							String::from("os"),
							serde_json::to_value(std::env::consts::OS)
								.map_err(VersionManagerError::SerdeJson)?,
						);

						let a =
							serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?;

						fs::write(path, a)
							.await
							.map_err(|e| FileIOError::from((path, e)))?;
					}

					(NodeConfigVersion::V4, NodeConfigVersion::V5) => {
						let mut config: Map<String, Value> =
							serde_json::from_slice(&fs::read(path).await.map_err(|e| {
								FileIOError::from((
									path,
									e,
									"Failed to read node config file for migration",
								))
							})?)
							.map_err(VersionManagerError::SerdeJson)?;

						config.insert(
							String::from("os"),
							serde_json::to_value(DeviceOS::from_env())
								.map_err(VersionManagerError::SerdeJson)?,
						);
						config.insert(
							String::from("hardware_model"),
							serde_json::to_value(
								HardwareModel::try_get().unwrap_or(HardwareModel::Other),
							)
							.map_err(VersionManagerError::SerdeJson)?,
						);

						config.remove("features");
						config.remove("auth_token");
						config.remove("sd_api_origin");
						config.remove("image_labeler_version");

						config.remove("id");
						config.insert(
							String::from("id"),
							serde_json::to_value(DevicePubId::from(Uuid::now_v7()))
								.map_err(VersionManagerError::SerdeJson)?,
						);

						fs::write(
							path,
							serde_json::to_vec(&config).map_err(VersionManagerError::SerdeJson)?,
						)
						.await
						.map_err(|e| {
							FileIOError::from((path, e, "Failed to write back updated config"))
						})?;
					}

					_ => {
						error!(current_version = ?current, "Node config version is not handled;");
						return Err(VersionManagerError::UnexpectedMigration {
							current_version: current.int_value(),
							next_version: next.int_value(),
						}
						.into());
					}
				}

				Ok(())
			},
		)
		.await
	}

	async fn save(&self, path: impl AsRef<Path>) -> Result<(), NodeConfigError> {
		let path = path.as_ref();
		fs::write(path, serde_json::to_vec(self)?)
			.await
			.map_err(|e| FileIOError::from((path, e)))?;

		Ok(())
	}
}

pub struct Manager {
	config: RwLock<NodeConfig>,
	data_directory_path: PathBuf,
	config_file_path: PathBuf,
	preferences_watcher_tx: watch::Sender<NodePreferences>,
}

impl Manager {
	/// new will create a new NodeConfigManager with the given path to the config file.
	pub(crate) async fn new(
		data_directory_path: impl AsRef<Path>,
	) -> Result<Arc<Self>, NodeConfigError> {
		let data_directory_path = data_directory_path.as_ref().to_path_buf();
		let config_file_path = data_directory_path.join(NODE_STATE_CONFIG_NAME);

		let config = NodeConfig::load(&config_file_path).await?;

		let (preferences_watcher_tx, _preferences_watcher_rx) =
			watch::channel(config.preferences.clone());

		Ok(Arc::new(Self {
			config: RwLock::new(config),
			data_directory_path,
			config_file_path,
			preferences_watcher_tx,
		}))
	}

	/// get will return the current NodeConfig in a read only state.
	pub(crate) async fn get(&self) -> NodeConfig {
		self.config.read().await.clone()
	}

	/// data_directory returns the path to the directory storing the configuration data.
	pub(crate) fn data_directory(&self) -> PathBuf {
		self.data_directory_path.clone()
	}

	/// write allows the user to update the configuration. This is done in a closure while a Mutex lock is held so that the user can't cause a race condition if the config were to be updated in multiple parts of the app at the same time.
	pub(crate) async fn write<F: FnOnce(&mut NodeConfig)>(
		&self,
		mutation_fn: F,
	) -> Result<NodeConfig, NodeConfigError> {
		let mut config = self.config.write().await;

		mutation_fn(&mut config);

		self.preferences_watcher_tx.send_if_modified(|current| {
			let modified = current != &config.preferences;
			if modified {
				*current = config.preferences.clone();
			}
			modified
		});

		config
			.save(&self.config_file_path)
			.await
			.map(|()| config.clone())
	}

	/// update_preferences allows the user to update the preferences of the node
	pub(crate) async fn update_preferences(
		&self,
		update_fn: impl FnOnce(&mut NodePreferences),
	) -> Result<(), NodeConfigError> {
		let mut config = self.config.write().await;

		update_fn(&mut config.preferences);

		self.preferences_watcher_tx
			.send_replace(config.preferences.clone());

		config.save(&self.config_file_path).await
	}
}

#[derive(Error, Debug)]
pub enum NodeConfigError {
	#[error(transparent)]
	SerdeJson(#[from] serde_json::Error),
	#[error(transparent)]
	VersionManager(#[from] VersionManagerError<NodeConfigVersion>),
	#[error(transparent)]
	FileIO(#[from] FileIOError),
}
