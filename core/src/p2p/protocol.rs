use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use sd_p2p::{
	proto::{decode, encode},
	spaceblock::{Range, SpaceblockRequests, SpaceblockRequestsError},
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
	#[error("error reading header file: {0}")]
	HeaderFile(decode::Error),
	#[error("error invalid header file discriminator '{0}'")]
	HeaderFileDiscriminatorInvalid(u8),
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
				id: decode::uuid(stream)
					.await
					.map_err(HeaderError::HeaderFile)?,
				library_id: decode::uuid(stream)
					.await
					.map_err(HeaderError::HeaderFile)?,
				file_path_id: decode::uuid(stream)
					.await
					.map_err(HeaderError::HeaderFile)?,
				range: match stream
					.read_u8()
					.await
					.map_err(|err| HeaderError::HeaderFile(err.into()))?
				{
					0 => Range::Full,
					1 => {
						let start = stream
							.read_u64_le()
							.await
							.map_err(|err| HeaderError::HeaderFile(err.into()))?;
						let end = stream
							.read_u64_le()
							.await
							.map_err(|err| HeaderError::HeaderFile(err.into()))?;
						Range::Partial(start..end)
					}
					i => return Err(HeaderError::HeaderFileDiscriminatorInvalid(i)),
				},
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
