use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use sd_p2p::{
	proto::{decode, encode},
	spaceblock::{SpaceblockRequest, SpacedropRequestError},
	spacetime::UnicastStream,
	spacetunnel::RemoteIdentity,
};

use crate::node::Platform;

/// TODO
#[derive(Debug, PartialEq, Eq)]
pub enum Header {
	// TODO: Split out cause this is a broadcast
	Ping,
	Spacedrop(SpaceblockRequest),
	Pair(Uuid),
	Sync(Uuid),
}

#[derive(Debug, Error)]
pub enum HeaderError {
	#[error("io error reading discriminator: {0}")]
	DiscriminatorIo(std::io::Error),
	#[error("invalid discriminator '{0}'")]
	DiscriminatorInvalid(u8),
	#[error("error reading spacedrop request: {0}")]
	Pairing(decode::Error),
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
			2 => Ok(Self::Pair(
				decode::uuid(stream).await.map_err(HeaderError::Pairing)?,
			)),
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
			Self::Pair(uuid) => {
				let mut bytes = vec![2];
				encode::uuid(&mut bytes, uuid);
				bytes
			}
			Self::Sync(uuid) => {
				let mut bytes = vec![3];
				encode::uuid(&mut bytes, uuid);
				bytes
			}
		}
	}
}

/// is shared between nodes during pairing and contains the information to identify the node.
#[derive(Debug, PartialEq, Eq)]
pub struct NodeLibraryPairingInformation {
	pub instance_id: Uuid,
	pub instance_public_key: RemoteIdentity,

	pub node_id: Uuid,
	pub node_name: String,
	pub node_platform: Platform,

	pub library_id: Uuid,
	pub library_name: String,
	pub library_description: Option<String>,
}

impl NodeLibraryPairingInformation {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, (&'static str, decode::Error)> {
		Ok(Self {
			instance_id: decode::uuid(stream).await.map_err(|e| ("instance_id", e))?,
			instance_public_key: decode::buf(stream)
				.await
				.and_then(|buf| Ok(RemoteIdentity::from_bytes(&buf)?))
				.map_err(|e| ("library_public_key", e))?,

			node_id: decode::uuid(stream).await.map_err(|e| ("node_id", e))?,
			node_name: decode::string(stream).await.map_err(|e| ("node_name", e))?,
			node_platform: stream
				.read_u8()
				.await
				.map(|b| Platform::try_from(b).unwrap_or(Platform::Unknown))
				.map_err(|e| ("node_platform", e.into()))?,

			library_id: decode::uuid(stream).await.map_err(|e| ("library_id", e))?,
			library_name: decode::string(stream)
				.await
				.map_err(|e| ("library_name", e))?,
			library_description: match decode::string(stream)
				.await
				.map_err(|e| ("library_description", e))?
			{
				s if s == "" => None,
				s => Some(s),
			},
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let Self {
			instance_id,
			instance_public_key,

			node_id,
			node_name,
			node_platform,
			library_id,
			library_name,
			library_description,
		} = self;

		let mut buf = Vec::new();

		encode::uuid(&mut buf, instance_id);
		encode::buf(&mut buf, &instance_public_key.to_bytes());

		encode::uuid(&mut buf, node_id);
		encode::string(&mut buf, node_name);
		buf.push(*node_platform as u8);

		encode::uuid(&mut buf, library_id);
		encode::string(&mut buf, library_name);

		encode::string(
			&mut buf,
			library_description.as_deref().unwrap_or_else(|| ""),
		);

		buf
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sd_p2p::spacetunnel::Identity;

	#[tokio::test]
	async fn test_node_information() {
		let original = NodeLibraryPairingInformation {
			instance_id: Uuid::new_v4(),
			instance_public_key: Identity::new().to_remote_identity(),

			node_id: Uuid::new_v4(),
			node_name: "Node Name".into(),
			node_platform: Platform::current(),

			library_id: Uuid::new_v4(),
			library_name: "Library Name".into(),
			library_description: Some("Library Description".into()),
		};

		let buf = original.to_bytes();
		let mut cursor = std::io::Cursor::new(buf);
		let info = NodeLibraryPairingInformation::from_stream(&mut cursor)
			.await
			.unwrap();

		assert_eq!(original, info);

		let original = NodeLibraryPairingInformation {
			library_description: None,
			..original
		};

		let buf = original.to_bytes();
		let mut cursor = std::io::Cursor::new(buf);
		let info = NodeLibraryPairingInformation::from_stream(&mut cursor)
			.await
			.unwrap();

		assert_eq!(original, info);
	}

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
