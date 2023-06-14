use rand_core::OsRng;

/// TODO
pub struct Identity(ed25519_dalek::Keypair);

impl Identity {
	pub fn new() -> Self {
		Self(ed25519_dalek::Keypair::generate(&mut OsRng))
	}

	pub fn from_bytes(bytes: &[u8]) -> Result<Self, ed25519_dalek::ed25519::Error> {
		Ok(Self(ed25519_dalek::Keypair::from_bytes(bytes)?))
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		self.0.to_bytes().to_vec()
	}
}
