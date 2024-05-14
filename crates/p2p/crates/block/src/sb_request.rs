use std::io;

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

use sd_p2p_proto::{decode, encode};

use super::BlockSize;

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Range {
	/// Request the entire file
	Full,
	/// Partial range
	Partial(std::ops::Range<u64>),
}

impl Range {
	// TODO: Per field and proper error handling
	pub async fn from_stream(stream: &mut (impl AsyncRead + Unpin)) -> std::io::Result<Self> {
		match stream.read_u8().await? {
			0 => Ok(Self::Full),
			1 => {
				let start = stream.read_u64_le().await?;
				let end = stream.read_u64_le().await?;
				Ok(Self::Partial(start..end))
			}
			_ => Err(io::Error::new(
				io::ErrorKind::Other,
				"Invalid range discriminator",
			)),
		}
	}

	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();

		match self {
			Self::Full => buf.push(0),
			Self::Partial(range) => {
				buf.push(1);
				buf.extend_from_slice(&range.start.to_le_bytes());
				buf.extend_from_slice(&range.end.to_le_bytes());
			}
		}
		buf
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceblockRequests {
	pub id: Uuid,
	pub block_size: BlockSize,
	pub requests: Vec<SpaceblockRequest>,
}

#[derive(Debug, Error)]
pub enum SpaceblockRequestsError {
	#[error("SpaceblockRequestsError::Id({0:?})")]
	Id(#[from] decode::Error),
	#[error("SpaceblockRequestsError::InvalidLen({0})")]
	InvalidLen(std::io::Error),
	#[error("SpaceblockRequestsError::SpaceblockRequest({0:?})")]
	SpaceblockRequest(#[from] SpaceblockRequestError),
	#[error("SpaceblockRequestsError::BlockSize({0:?})")]
	BlockSize(std::io::Error),
}

impl SpaceblockRequests {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, SpaceblockRequestsError> {
		let id = decode::uuid(stream)
			.await
			.map_err(SpaceblockRequestsError::Id)?;

		let block_size = BlockSize::from_stream(stream)
			.await
			.map_err(SpaceblockRequestsError::BlockSize)?;

		let size = stream
			// Max of 255 files in one request
			.read_u8()
			.await
			.map_err(SpaceblockRequestsError::InvalidLen)?;

		let mut requests = Vec::new();
		for _i in 0..size {
			requests.push(SpaceblockRequest::from_stream(stream).await?);
		}

		Ok(Self {
			id,
			block_size,
			requests,
		})
	}

	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		let Self {
			id,
			block_size,
			requests,
		} = self;
		assert!(
			requests.len() <= 255,
			"Can't Spacedrop more than 255 files at once!"
		);

		let mut buf = vec![];
		encode::uuid(&mut buf, id);
		buf.append(&mut block_size.to_bytes().to_vec());
		buf.push(requests.len() as u8);
		for request in requests {
			buf.extend_from_slice(&request.to_bytes());
		}
		buf
	}
}

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceblockRequest {
	pub name: String,
	pub size: u64,
	// TODO: Include file permissions
	pub range: Range,
}

#[derive(Debug, Error)]
pub enum SpaceblockRequestError {
	#[error("SpaceblockRequestError::Name({0})")]
	Name(decode::Error),
	#[error("SpaceblockRequestError::Size({0})")]
	Size(std::io::Error),
	// TODO: From outside. Probs remove?
	#[error("SpaceblockRequestError::RangeError({0:?})")]
	RangeError(io::Error),
}

impl SpaceblockRequest {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, SpaceblockRequestError> {
		let name = decode::string(stream)
			.await
			.map_err(SpaceblockRequestError::Name)?;

		let size = stream
			.read_u64_le()
			.await
			.map_err(SpaceblockRequestError::Size)?;

		Ok(Self {
			name,
			size,
			range: Range::from_stream(stream)
				.await
				.map_err(SpaceblockRequestError::Size)?,
		})
	}

	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();

		encode::string(&mut buf, &self.name);
		buf.extend_from_slice(&self.size.to_le_bytes());
		buf.extend_from_slice(&self.range.to_bytes());
		buf
	}
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use super::*;

	#[tokio::test]
	async fn test_range() {
		let req = Range::Full;
		let bytes = req.to_bytes();
		let req2 = Range::from_stream(&mut Cursor::new(bytes)).await.unwrap();
		assert_eq!(req, req2);

		let req = Range::Partial(0..10);
		let bytes = req.to_bytes();
		let req2 = Range::from_stream(&mut Cursor::new(bytes)).await.unwrap();
		assert_eq!(req, req2);
	}

	#[tokio::test]
	async fn test_spaceblock_requests_empty() {
		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size: BlockSize::from_file_size(42069),
			requests: vec![],
		};

		let bytes = req.to_bytes();
		let req2 = SpaceblockRequests::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);
	}

	#[tokio::test]
	async fn test_spaceblock_requests_one() {
		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size: BlockSize::from_file_size(42069),
			requests: vec![SpaceblockRequest {
				name: "Demo".to_string(),
				size: 42069,
				range: Range::Full,
			}],
		};

		let bytes = req.to_bytes();
		let req2 = SpaceblockRequests::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);

		let req = SpaceblockRequest {
			name: "Demo".to_string(),
			size: 42069,
			range: Range::Partial(0..420),
		};

		let bytes = req.to_bytes();
		let req2 = SpaceblockRequest::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);
	}

	#[tokio::test]
	async fn test_spaceblock_requests_many() {
		let req = SpaceblockRequests {
			id: Uuid::new_v4(),
			block_size: BlockSize::from_file_size(42069),
			requests: vec![
				SpaceblockRequest {
					name: "Demo".to_string(),
					size: 42069,
					range: Range::Full,
				},
				SpaceblockRequest {
					name: "Demo2".to_string(),
					size: 420,
					range: Range::Full,
				},
			],
		};

		let bytes = req.to_bytes();
		let req2 = SpaceblockRequests::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);
	}
}
