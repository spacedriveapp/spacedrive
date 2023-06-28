use std::string::FromUtf8Error;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use sd_p2p::{
	spaceblock::{SpaceblockRequest, SpacedropRequestError},
	spacetime::SpaceTimeStream,
	spacetunnel::{IdentityErr, RemoteIdentity},
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

#[derive(Debug, Error)]
pub enum NodeInformationError {
	#[error("io error decoding node information library pub_id: {0}")]
	ErrorDecodingUuid(std::io::Error),
	#[error("error formatting node information library pub_id: {0}")]
	UuidFormatError(uuid::Error),
	#[error("io error reading node information library name length: {0}")]
	NameLenIoError(std::io::Error),
	#[error("io error decoding node information library name: {0}")]
	ErrorDecodingName(std::io::Error),
	#[error("error formatting node information library name: {0}")]
	NameFormatError(FromUtf8Error),
	#[error("io error reading node information public key length: {0}")]
	PublicKeyLenIoError(std::io::Error),
	#[error("io error decoding node information public key: {0}")]
	ErrorDecodingPublicKey(std::io::Error),
	#[error("error decoding public key: {0}")]
	ErrorParsingPublicKey(#[from] IdentityErr),
	#[error("io error reading node information platform id: {0}")]
	PlatformIdError(std::io::Error),
}

/// is shared between nodes during pairing and contains the information to identify the node.
#[derive(Debug, PartialEq, Eq)]
pub struct NodeInformation {
	pub pub_id: Uuid,
	pub name: String,
	pub public_key: RemoteIdentity,
	pub platform: Platform,
}

impl NodeInformation {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, NodeInformationError> {
		let pub_id = {
			let mut buf = vec![0u8; 16];
			stream
				.read_exact(&mut buf)
				.await
				.map_err(NodeInformationError::ErrorDecodingUuid)?;

			Uuid::from_slice(&buf).map_err(NodeInformationError::UuidFormatError)?
		};

		let name = {
			let len = stream
				.read_u16_le()
				.await
				.map_err(NodeInformationError::NameLenIoError)?;

			let mut buf = vec![0u8; len as usize];
			stream
				.read_exact(&mut buf)
				.await
				.map_err(NodeInformationError::ErrorDecodingName)?;

			String::from_utf8(buf).map_err(NodeInformationError::NameFormatError)?
		};

		let public_key = {
			let len = stream
				.read_u16_le()
				.await
				.map_err(NodeInformationError::PublicKeyLenIoError)?;

			let mut buf = vec![0u8; len as usize];
			stream
				.read_exact(&mut buf)
				.await
				.map_err(NodeInformationError::ErrorDecodingPublicKey)?;

			RemoteIdentity::from_bytes(&buf)?
		};

		let platform = stream
			.read_u8()
			.await
			.map_err(NodeInformationError::PlatformIdError)?;

		Ok(Self {
			pub_id,
			name,
			public_key,
			platform: Platform::try_from(platform).unwrap_or(Platform::Unknown),
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();

		// Pub id
		buf.extend(self.pub_id.as_bytes());

		// Name
		let len_buf = (self.name.len() as u16).to_le_bytes();
		if self.name.len() > u16::MAX as usize {
			panic!("Name is too long!"); // TODO: Error handling
		}
		buf.extend_from_slice(&len_buf);
		buf.extend(self.name.as_bytes());

		// Public key // TODO: Can I use a fixed size array?
		let pk = self.public_key.to_bytes();
		let len_buf = (pk.len() as u16).to_le_bytes();
		if pk.len() > u16::MAX as usize {
			panic!("Public key is too long!"); // TODO: Error handling
		}
		buf.extend_from_slice(&len_buf);
		buf.extend(pk);

		// Platform
		buf.push(self.platform as u8);

		buf
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sd_p2p::spacetunnel::Identity;

	#[tokio::test]
	async fn test_node_information() {
		let original = NodeInformation {
			pub_id: Uuid::new_v4(),
			name: "Name".into(),
			public_key: Identity::new().to_remote_identity(),
			platform: Platform::current(),
		};

		let buf = original.to_bytes();
		let mut cursor = std::io::Cursor::new(buf);
		let info = NodeInformation::from_stream(&mut cursor).await.unwrap();

		assert_eq!(original, info);
	}

	// TODO: Unit test it because binary protocols are error prone
	// #[test]
	// fn test_proto() {
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
	// }
}
