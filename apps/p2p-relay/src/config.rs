use std::{borrow::Cow, path::Path};

use libp2p::identity::Keypair;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
	// Unique ID of this relay server.
	pub id: Uuid,
	// URL of the cloud API.
	#[serde(skip_serializing_if = "Option::is_none")]
	api_url: Option<String>,
	// Secret used for authenticating with cloud backend.
	pub p2p_secret: String,
	// Port to listen on.
	#[serde(skip_serializing_if = "Option::is_none")]
	port: Option<u16>,
	// Private/public keypair to use for the relay.
	#[serde(with = "keypair")]
	pub keypair: Keypair,
}

impl Config {
	pub fn init(
		path: impl AsRef<Path>,
		p2p_secret: String,
	) -> Result<Self, Box<dyn std::error::Error>> {
		let config = Self {
			id: Uuid::new_v4(),
			api_url: None,
			p2p_secret,
			port: None,
			keypair: Keypair::generate_ed25519(),
		};
		std::fs::write(path, serde_json::to_string_pretty(&config)?)?;
		Ok(config)
	}

	pub fn load(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
		let config = std::fs::read_to_string(path)?;
		Ok(serde_json::from_str(&config)?)
	}

	pub fn api_url(&self) -> Cow<'_, str> {
		match self.api_url {
			Some(ref url) => Cow::Borrowed(url),
			None => Cow::Borrowed("https://app.spacedrive.com"),
		}
	}

	pub fn port(&self) -> u16 {
		self.port.unwrap_or(7373) // TODO: Should we use HTTPS port to avoid strict internet filters???
	}
}

mod keypair {
	use libp2p::identity::Keypair;
	use serde::{de::Error, Deserialize, Deserializer, Serializer};

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Keypair, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		let bytes = hex::decode(s).map_err(D::Error::custom)?;
		Keypair::from_protobuf_encoding(bytes.as_slice()).map_err(D::Error::custom)
	}

	pub fn serialize<S: Serializer>(v: &Keypair, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_str(&hex::encode(
			v.to_protobuf_encoding().expect("invalid keypair type"),
		))
	}
}
