use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::proto::{decode, encode};

use super::BlockSize;

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceblockRequest {
	pub name: String,
	pub size: u64,
	// TODO: Include file permissions
	pub block_size: BlockSize,
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
		})
	}

	pub fn to_bytes(&self) -> Vec<u8> {
		let Self {
			name,
			size,
			block_size,
		} = self;
		let mut buf = Vec::new();

		encode::string(&mut buf, name);
		buf.extend_from_slice(&self.size.to_le_bytes());

		buf
	}
}
