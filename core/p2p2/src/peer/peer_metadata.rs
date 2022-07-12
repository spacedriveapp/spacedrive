use std::{collections::HashMap, env, str::FromStr};

use sd_tunnel_utils::PeerId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, TS, Serialize, Deserialize)]
pub enum OperationSystem {
	Windows,
	Linux,
	MacOS,
	Other(String),
}

impl OperationSystem {
	pub fn get_os() -> Self {
		match env::consts::OS {
			"windows" => OperationSystem::Windows,
			"macos" => OperationSystem::MacOS,
			"linux" => OperationSystem::Linux,
			platform => OperationSystem::Other(platform.into()),
		}
	}
}

impl Into<String> for OperationSystem {
	fn into(self) -> String {
		match self {
			OperationSystem::Windows => "w".to_string(),
			OperationSystem::Linux => "l".to_string(),
			OperationSystem::MacOS => "m".to_string(),
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
			_ => Ok(OperationSystem::Other(chars.as_str().to_string())),
		}
	}
}

/// PeerMetadata represents public metadata about a peer. This is found through the discovery process.
#[derive(Debug, Clone, TS, Serialize, Deserialize)]
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
				.unwrap_or(peer_id.to_string()),
			operating_system: hashmap
				.get("os")
				.map(|v| Some(v.parse().unwrap()))
				.unwrap_or(None),
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
