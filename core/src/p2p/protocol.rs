use sd_prisma::prisma::instance;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use sd_p2p::{
	proto::{decode, encode},
	spaceblock::{SpaceblockRequest, SpacedropRequestError},
	spacetime::UnicastStream,
	spacetunnel::RemoteIdentity,
};

use crate::{library::LibraryConfig, node::Platform};

/// TODO
#[derive(Debug, PartialEq, Eq)]
pub enum Header {
	// TODO: Split out cause this is a broadcast
	Ping,
	Spacedrop(SpaceblockRequest),
	Pair,
	Sync(Uuid),
}

#[derive(Debug, Error)]
pub enum HeaderError {
	#[error("io error reading discriminator: {0}")]
	DiscriminatorIo(std::io::Error),
	#[error("invalid discriminator '{0}'")]
	DiscriminatorInvalid(u8),
	#[error("error reading spacedrop request: {0}")]
	SpacedropRequest(#[from] SpacedropRequestError),
	#[error("error reading sync request: {0}")]
	SyncRequest(decode::Error),
}

impl Header {
	pub async fn from_stream(stream: &mut UnicastStream) -> Result<Self, HeaderError> {
		let discriminator = stream
			.read_u8()
			.await
			.map_err(HeaderError::DiscriminatorIo)?;

		match discriminator {
			0 => Ok(Self::Spacedrop(
				SpaceblockRequest::from_stream(stream).await?,
			)),
			1 => Ok(Self::Ping),
			2 => Ok(Self::Pair),
			3 => Ok(Self::Sync(
				decode::uuid(stream)
					.await
					.map_err(HeaderError::SyncRequest)?,
			)),
			d => Err(HeaderError::DiscriminatorInvalid(d)),
		}
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		match self {
			Self::Spacedrop(transfer_request) => {
				let mut bytes = vec![0];
				bytes.extend_from_slice(&transfer_request.to_bytes());
				bytes
			}
			Self::Ping => vec![1],
			Self::Pair => vec![2],
			Self::Sync(uuid) => {
				let mut bytes = vec![3];
				encode::uuid(&mut bytes, uuid);
				bytes
			}
		}
	}
}

#[cfg(test)]
mod tests {
	// use super::*;

	#[test]
	fn test_header() {
		// TODO: Finish this

		// 	assert_eq!(
		// 		Header::from_bytes(&Header::Ping.to_bytes()),
		// 		Ok(Header::Ping)
		// 	);

		// 	assert_eq!(
		// 		Header::from_bytes(&Header::Spacedrop.to_bytes()),
		// 		Ok(Header::Spacedrop)
		// 	);

		// 	let uuid = Uuid::new_v4();
		// 	assert_eq!(
		// 		Header::from_bytes(&Header::Sync(uuid).to_bytes()),
		// 		Ok(Header::Sync(uuid))
		// 	);
	}
}
