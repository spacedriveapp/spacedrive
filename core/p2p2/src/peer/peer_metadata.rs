use std::collections::HashMap;

use sd_tunnel_utils::PeerId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// PeerMetadata represents public metadata about a peer. This is found through the discovery process.
#[derive(Debug, Clone, TS, Serialize, Deserialize)]
pub struct PeerMetadata {
	pub name: String,
	pub version: Option<String>,
}

impl PeerMetadata {
	pub fn from_hashmap(peer_id: &PeerId, hashmap: &HashMap<String, String>) -> Self {
		Self {
			name: hashmap
				.get("name")
				.map(|v| v.to_string())
				.unwrap_or(peer_id.to_string()),
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
