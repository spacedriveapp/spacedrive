use sd_p2p_block::{Range, SpaceblockRequests, SpaceblockRequestsError};
use sd_p2p_proto::{decode, encode};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

/// TODO
#[derive(Debug, PartialEq, Eq)]
pub enum Header {
	/// Basic pin protocol for demonstrating the P2P system
	Ping,
	/// Spacedrop file sending
	Spacedrop(SpaceblockRequests),
	/// Used for sending sync messages between nodes.
	Sync,
	// A HTTP server used for rspc requests and streaming files
	RspcRemote,
	// Request a file within a library
	// We don't include a library ID here as it's taken care of by `sd_p2p_tunnel::Tunnel`.
	LibraryFile {
		file_path_id: Uuid,
		range: Range,
	},
}

#[derive(Debug, Error)]
pub enum HeaderError {
	#[error("io error reading discriminator: {0}")]
	DiscriminatorIo(std::io::Error),
	#[error("invalid discriminator '{0}'")]
	DiscriminatorInvalid(u8),
	#[error("error reading spacedrop request: {0}")]
	SpacedropRequest(#[from] SpaceblockRequestsError),
	#[error("error with library file decode '{0}'")]
	LibraryFileDecodeError(decode::Error),
	#[error("error with library file io '{0}'")]
	LibraryFileIoError(std::io::Error),
	#[error("invalid range discriminator for library file req '{0}'")]
	LibraryDiscriminatorInvalid(u8),
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
			3 => Ok(Self::Sync),
			5 => Ok(Self::RspcRemote),
			6 => Ok(Self::LibraryFile {
				file_path_id: decode::uuid(stream)
					.await
					.map_err(HeaderError::LibraryFileDecodeError)?,
				range: match stream
					.read_u8()
					.await
					.map_err(HeaderError::LibraryFileIoError)?
				{
					0 => Range::Full,
					1 => {
						let start = stream
							.read_u64_le()
							.await
							.map_err(HeaderError::LibraryFileIoError)?;
						let end = stream
							.read_u64_le()
							.await
							.map_err(HeaderError::LibraryFileIoError)?;
						Range::Partial(start..end)
					}
					d => return Err(HeaderError::LibraryDiscriminatorInvalid(d)),
				},
			}),
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
			Self::Sync => vec![3],
			Self::RspcRemote => vec![5],
			Self::LibraryFile {
				file_path_id,
				range,
			} => {
				let mut buf = vec![6];
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
