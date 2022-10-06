use std::{collections::HashMap, env, str::FromStr};

use sd_tunnel_utils::PeerId;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Represents the operating system which the remote peer is running.
/// This is not used internally and predominantly is designed to be used for display purposes by the embedding application.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub enum OperationSystem {
	Windows,
	Linux,
	MacOS,
	IOS,
	Android,
	Other(String),
}

impl OperationSystem {
	pub fn get_os() -> Self {
		match env::consts::OS {
			"windows" => OperationSystem::Windows,
			"macos" => OperationSystem::MacOS,
			"linux" => OperationSystem::Linux,
			"ios" => OperationSystem::IOS,
			"android" => OperationSystem::Android,
			platform => OperationSystem::Other(platform.into()),
		}
	}
}

impl From<OperationSystem> for String {
	fn from(os: OperationSystem) -> Self {
		match os {
			OperationSystem::Windows => "Windows".into(),
			OperationSystem::Linux => "Linux".into(),
			OperationSystem::MacOS => "MacOS".into(),
			OperationSystem::IOS => "IOS".into(),
			OperationSystem::Android => "Android".into(),
			OperationSystem::Other(s) => {
				let mut chars = s.chars();
				chars.next();
				chars.as_str().to_string()
			}
		}
	}
}

impl FromStr for OperationSystem {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut chars = s.chars();
		match chars.next() {
			Some('w') => Ok(OperationSystem::Windows),
			Some('l') => Ok(OperationSystem::Linux),
			Some('m') => Ok(OperationSystem::MacOS),
			Some('i') => Ok(OperationSystem::IOS),
			Some('a') => Ok(OperationSystem::Android),
			_ => Ok(OperationSystem::Other(chars.as_str().to_string())),
		}
	}
}

/// Represents public metadata about a peer. This is designed to hold information which is required among all applications using the P2P library.
/// This metadata is discovered through the discovery process or sent by the connecting device when establishing a new P2P connection.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct PeerMetadata {
	pub name: String,
	pub operating_system: Option<OperationSystem>,
	pub version: Option<String>,
}

impl PeerMetadata {
	pub fn from_hashmap(peer_id: &PeerId, hashmap: &HashMap<String, String>) -> Self {
		Self {
			name: hashmap
				.get("name")
				.map(|v| v.to_string())
				.unwrap_or_else(|| peer_id.to_string()),
			operating_system: hashmap.get("os").map(|v| v.parse().ok()).unwrap_or(None),
			version: hashmap.get("version").map(|v| v.to_string()),
		}
	}

	pub fn to_hashmap(self) -> HashMap<String, String> {
		let mut hashmap = HashMap::new();
		hashmap.insert("name".to_string(), self.name);
		if let Some(version) = self.version {
			hashmap.insert("version".to_string(), version);
		}
		hashmap
	}
}
