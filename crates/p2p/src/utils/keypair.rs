use libp2p::identity::ed25519::{self};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Keypair(ed25519::Keypair);

impl Keypair {
	pub fn generate() -> Self {
		Self(ed25519::Keypair::generate())
	}

	pub fn peer_id(&self) -> crate::PeerId {
		let pk: libp2p::identity::PublicKey = self.0.public().into();

		crate::PeerId(libp2p::PeerId::from_public_key(&pk))
	}

	// TODO: Maybe try and remove
	pub fn raw_peer_id(&self) -> libp2p::PeerId {
		let pk: libp2p::identity::PublicKey = self.0.public().into();

		libp2p::PeerId::from_public_key(&pk)
	}

	pub fn inner(&self) -> libp2p::identity::Keypair {
		self.0.clone().into()
	}
}

impl Serialize for Keypair {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.serialize_bytes(&self.0.to_bytes())
	}
}

impl<'de> Deserialize<'de> for Keypair {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let mut bytes = Vec::<u8>::deserialize(deserializer)?;
		Ok(Self(
			ed25519::Keypair::try_from_bytes(bytes.as_mut_slice())
				.map_err(serde::de::Error::custom)?,
		))
	}
}
