use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::proto::{decode, encode};

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
		match stream.read_u8().await.unwrap() {
			0 => Ok(Self::Full),
			1 => {
				let start = stream.read_u64_le().await.unwrap();
				let end = stream.read_u64_le().await.unwrap();
				Ok(Self::Partial(start..end))
			}
			_ => todo!(),
		}
	}

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

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceblockRequest {
	pub name: String,
	pub size: u64,
	// TODO: Include file permissions
	pub block_size: BlockSize,
	pub range: Range,
}

#[derive(Debug, Error)]
pub enum SpacedropRequestError {
	#[error("SpacedropRequestError::Name({0})")]
	Name(decode::Error),
	#[error("SpacedropRequestError::Size({0})")]
	Size(std::io::Error),
}

impl SpaceblockRequest {
	pub async fn from_stream(
		stream: &mut (impl AsyncRead + Unpin),
	) -> Result<Self, SpacedropRequestError> {
		let name = decode::string(stream)
			.await
			.map_err(SpacedropRequestError::Name)?;

		let size = stream
			.read_u64_le()
			.await
			.map_err(SpacedropRequestError::Size)?;
		let block_size = BlockSize::from_size(size); // TODO: Get from stream: stream.read_u8().await.map_err(|_| ())?; // TODO: Error handling

		Ok(Self {
			name,
			size,
			block_size,
			range: Range::from_stream(stream).await.unwrap(),
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let Self {
			name,
			size,
			block_size,
			range,
		} = self;
		let mut buf = Vec::new();

		encode::string(&mut buf, name);
		buf.extend_from_slice(&self.size.to_le_bytes());
		buf.extend_from_slice(&self.range.to_bytes());
		buf
	}
}
