use libp2p::identity::{ed25519, PublicKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Keypair(libp2p::identity::Keypair);

impl Keypair {
	pub fn generate() -> Self {
		Self(libp2p::identity::Keypair::generate_ed25519())
	}

	pub fn public(&self) -> PublicKey {
		self.0.public()
	}

	pub fn inner(&self) -> &libp2p::identity::Keypair {
		&self.0
	}
}

impl Serialize for Keypair {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		match &self.0 {
			libp2p::identity::Keypair::Ed25519(keypair) => {
				serializer.serialize_bytes(&keypair.encode())
			}
			_ => unreachable!(),
		}
	}
}

impl<'de> Deserialize<'de> for Keypair {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let mut bytes = Vec::<u8>::deserialize(deserializer)?;
		Ok(Self(libp2p::identity::Keypair::Ed25519(
			ed25519::Keypair::decode(bytes.as_mut_slice()).map_err(serde::de::Error::custom)?,
		)))
	}
}
