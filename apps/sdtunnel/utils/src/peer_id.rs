use std::{fmt, ops::Deref};

use ring::digest::digest;
use rustls::Certificate;
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

/// is a unique identifier for a peer. These are derived from the public key of the peer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Serialize, Deserialize, Type)]
pub struct PeerId(String);

impl PeerId {
	/// from_str attempts to load a PeerId from a string. It will return an error if the PeerId is invalid.
	pub fn from_string(id: String) -> Result<Self, PeerIdError> {
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

impl PartialEq<PeerId> for &PeerId {
	fn eq(&self, other: &PeerId) -> bool {
		self.0 == other.0
	}
}

impl fmt::Display for PeerId {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl Deref for PeerId {
	type Target = String;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

/// Represents an error that can occur when creating a [PeerId] from a string.
#[derive(Error, Debug)]
pub enum PeerIdError {
	#[error("the PeerId must be 40 chars in length")]
	InvalidLength,
	#[error("the PeerId must be alphanumeric")]
	InvalidCharacters,
}
