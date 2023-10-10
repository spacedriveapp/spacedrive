use sd_p2p::spacetunnel::{Identity, IdentityErr, RemoteIdentity};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityOrRemoteIdentityErr {
	#[error("IdentityErr({0})")]
	IdentityErr(#[from] IdentityErr),
	#[error("InvalidFormat")]
	InvalidFormat,
}

/// TODO
#[derive(Debug, PartialEq)]

pub enum IdentityOrRemoteIdentity {
	Identity(Identity),
	RemoteIdentity(RemoteIdentity),
}

impl IdentityOrRemoteIdentity {
	pub fn remote_identity(&self) -> RemoteIdentity {
		match self {
			Self::Identity(identity) => identity.to_remote_identity(),
			Self::RemoteIdentity(identity) => {
				RemoteIdentity::from_bytes(identity.to_bytes().as_slice()).expect("unreachable")
			}
		}
	}
}

impl IdentityOrRemoteIdentity {
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityOrRemoteIdentityErr> {
		match bytes[0] {
			b'I' => Ok(Self::Identity(Identity::from_bytes(&bytes[1..])?)),
			b'R' => Ok(Self::RemoteIdentity(RemoteIdentity::from_bytes(
				&bytes[1..],
			)?)),
			_ => Err(IdentityOrRemoteIdentityErr::InvalidFormat),
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::Identity(identity) => [&[b'I'], &*identity.to_bytes()].concat(),
			Self::RemoteIdentity(identity) => [[b'R'].as_slice(), &identity.to_bytes()].concat(),
		}
	}
}
