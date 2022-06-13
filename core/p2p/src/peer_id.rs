use std::fmt;

use ring::digest::digest;
use rustls::Certificate;
use thiserror::Error;

/// PeerId is a unique identifier for a peer. These are derived from the public key of the peer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd)]
pub struct PeerId(String);

impl PeerId {
	/// from_str attempts to load a PeerId from a string. It will return an error if the PeerId is invalid.
	pub fn from_str(id: String) -> Result<Self, PeerIdError> {
		if id.len() != 40 {
			return Err(PeerIdError::InvalidLength);
		} else if !id.chars().all(char::is_alphanumeric) {
			return Err(PeerIdError::InvalidCharacters);
		}
		Ok(Self(id))
	}

	/// from_cert will derive a [PeerId] from a [rustls::Certificate].
	pub fn from_cert(cert: &Certificate) -> Self {
		// SHA-1 is used due to the limitation of the length of a DNS record used for mDNS local network discovery.
		let peer_id = digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &cert.0)
			.as_ref()
			.iter()
			.map(|b| format!("{:02x}", b))
			.collect();

		Self(peer_id)
	}
}

impl fmt::Display for PeerId {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// PeerIdError is an error that can occur when creating a [PeerId] from a string.
#[derive(Error, Debug)]
pub enum PeerIdError {
	#[error("the PeerId must be 40 chars in length")]
	InvalidLength,
	#[error("the PeerId must be alphanumeric")]
	InvalidCharacters,
}
