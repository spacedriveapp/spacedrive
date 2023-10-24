use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use sd_p2p::{
	proto::{decode, encode},
	spaceblock::{Range, SpaceblockRequests, SpaceblockRequestsError},
	spacetunnel::RemoteIdentity,
};

#[derive(Debug, PartialEq, Eq)]
pub struct HeaderFile {
	// Request ID
	pub(crate) id: Uuid,
	pub(crate) library_id: Uuid,
	pub(crate) file_path_id: Uuid,
	pub(crate) range: Range,
}

/// TODO
#[derive(Debug, PartialEq, Eq)]
pub enum Header {
	// TODO: Split out cause this is a broadcast
	Ping,
	Spacedrop(SpaceblockRequests),
	Pair,
	Sync(Uuid),
	File(HeaderFile),
	// TODO: Remove need for this
	Connected(Vec<RemoteIdentity>),
}

#[derive(Debug, Error)]
pub enum HeaderError {
	#[error("io error reading discriminator: {0}")]
	DiscriminatorIo(std::io::Error),
	#[error("invalid discriminator '{0}'")]
	DiscriminatorInvalid(u8),
	#[error("error reading spacedrop request: {0}")]
	SpacedropRequest(#[from] SpaceblockRequestsError),
	#[error("error reading sync request: {0}")]
	SyncRequest(decode::Error),
}

impl Header {
	pub async fn from_stream(stream: &mut (impl AsyncRead + Unpin)) -> Result<Self, HeaderError> {
		let discriminator = stream
			.read_u8()
			.await
			.map_err(HeaderError::DiscriminatorIo)?;

		match discriminator {
			0 => Ok(Self::Spacedrop(
				SpaceblockRequests::from_stream(stream).await?,
			)),
			1 => Ok(Self::Ping),
			2 => Ok(Self::Pair),
			3 => Ok(Self::Sync(
				decode::uuid(stream)
					.await
					.map_err(HeaderError::SyncRequest)?,
			)),
			4 => Ok(Self::File(HeaderFile {
				id: decode::uuid(stream).await.unwrap(),
				library_id: decode::uuid(stream).await.unwrap(),
				file_path_id: decode::uuid(stream).await.unwrap(),
				range: match stream.read_u8().await.unwrap() {
					0 => Range::Full,
					1 => {
						let start = stream.read_u64_le().await.unwrap();
						let end = stream.read_u64_le().await.unwrap();
						Range::Partial(start..end)
					}
					_ => todo!(),
				},
			})),
			// TODO: Error handling
			255 => Ok(Self::Connected({
				let len = stream.read_u16_le().await.unwrap();
				let mut identities = Vec::with_capacity(len as usize);
				for _ in 0..len {
					identities.push(
						RemoteIdentity::from_bytes(&decode::buf(stream).await.unwrap()).unwrap(),
					);
				}
				identities
			})),
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
			Self::File(HeaderFile {
				id,
				library_id,
				file_path_id,
				range,
			}) => {
				let mut buf = vec![4];
				encode::uuid(&mut buf, id);
				encode::uuid(&mut buf, library_id);
				encode::uuid(&mut buf, file_path_id);
				buf.extend_from_slice(&range.to_bytes());
				buf
			}

			Self::Connected(remote_identities) => {
				let mut bytes = vec![255];
				if remote_identities.len() > u16::MAX as usize {
					panic!("Buf is too long!"); // TODO: Chunk this so it will never error
				}
				bytes.extend((remote_identities.len() as u16).to_le_bytes());
				for identity in remote_identities {
					encode::buf(&mut bytes, &identity.to_bytes());
				}
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
