use thiserror::Error;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use sd_p2p::{
	spaceblock::{SpaceblockRequest, SpacedropRequestError},
	spacetime::SpaceTimeStream,
};

use crate::node::Platform;

/// TODO
#[derive(Debug, PartialEq, Eq)]
pub enum Header {
	Ping,
	Spacedrop(SpaceblockRequest),
	Pair(Uuid),
	Sync(Uuid),
}

#[derive(Debug, Error)]
pub enum SyncRequestError {
	#[error("io error reading library id: {0}")]
	LibraryIdIoError(std::io::Error),
	#[error("io error decoding library id: {0}")]
	ErrorDecodingLibraryId(uuid::Error),
	#[error("io error reading sync payload len: {0}")]
	PayloadLenIoError(std::io::Error),
}

#[derive(Debug, Error)]
pub enum HeaderError {
	#[error("io error reading discriminator: {0}")]
	DiscriminatorIoError(std::io::Error),
	#[error("invalid discriminator '{0}'")]
	InvalidDiscriminator(u8),
	#[error("error reading spacedrop request: {0}")]
	SpacedropRequestError(#[from] SpacedropRequestError),
	#[error("error reading sync request: {0}")]
	SyncRequestError(#[from] SyncRequestError),
	#[error("invalid request. Spacedrop requires a unicast stream!")]
	SpacedropOverMulticastIsForbidden,
}

impl Header {
	pub async fn from_stream(stream: &mut SpaceTimeStream) -> Result<Self, HeaderError> {
		let discriminator = stream
			.read_u8()
			.await
			.map_err(HeaderError::DiscriminatorIoError)?;

		match discriminator {
			0 => match stream {
				SpaceTimeStream::Unicast(stream) => Ok(Self::Spacedrop(
					SpaceblockRequest::from_stream(stream).await?,
				)),
				_ => Err(HeaderError::SpacedropOverMulticastIsForbidden),
			},
			1 => Ok(Self::Ping),
			2 => {
				let mut uuid = [0u8; 16];
				stream
					.read_exact(&mut uuid)
					.await
					.map_err(SyncRequestError::LibraryIdIoError)?;

				Ok(Self::Pair(
					Uuid::from_slice(&uuid).map_err(SyncRequestError::ErrorDecodingLibraryId)?,
				))
			}
			3 => {
				let mut uuid = [0u8; 16];
				stream
					.read_exact(&mut uuid)
					.await
					.map_err(SyncRequestError::LibraryIdIoError)?;

				Ok(Self::Sync(
					Uuid::from_slice(&uuid).map_err(SyncRequestError::ErrorDecodingLibraryId)?,
				))
			}
			d => Err(HeaderError::InvalidDiscriminator(d)),
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
			Self::Pair(library_id) => {
				let mut bytes = vec![2];
				bytes.extend_from_slice(library_id.as_bytes());
				bytes
			}
			Self::Sync(uuid) => {
				let mut bytes = vec![3];
				bytes.extend_from_slice(uuid.as_bytes());
				bytes
			}
		}
	}
}

/// is shared between nodes during pairing and contains the information to identify the node.
pub struct NodeInformation {
	pub name: String,
	pub public_key: Vec<u8>,
	pub platform: Platform,
}

impl NodeInformation {
	pub fn to_bytes(&self) -> Vec<u8> {
		let buf = Vec::new();

		// TODO
		// buf.extend_from_slice((self.name.len() as u16).as_le_bytes());

		buf
	}
}

// TODO: Unit test it because binary protocols are error prone
// #[cfg(test)]
// mod tests {
// 	use super::*;

// 	#[test]
// 	fn test_proto() {
// 		assert_eq!(
// 			Header::from_bytes(&Header::Ping.to_bytes()),
// 			Ok(Header::Ping)
// 		);

// 		assert_eq!(
// 			Header::from_bytes(&Header::Spacedrop.to_bytes()),
// 			Ok(Header::Spacedrop)
// 		);

// 		let uuid = Uuid::new_v4();
// 		assert_eq!(
// 			Header::from_bytes(&Header::Sync(uuid).to_bytes()),
// 			Ok(Header::Sync(uuid))
// 		);
// 	}
// }
