use std::io;

use tokio::io::{AsyncRead, AsyncReadExt};

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockSize(u32); // Max block size is gonna be 3.9GB which is stupidly overkill

impl BlockSize {
	// TODO: Validating `BlockSize` are multiple of 2, i think. Idk why but BEP does it.

	pub async fn from_stream(stream: &mut (impl AsyncRead + Unpin)) -> io::Result<Self> {
		stream.read_u32_le().await.map(Self)
	}

	pub fn to_bytes(&self) -> [u8; 4] {
		self.0.to_le_bytes()
	}

	pub fn from_size(size: u64) -> Self {
		// TODO: Something like: https://docs.syncthing.net/specs/bep-v1.html#selection-of-block-size
		Self(131072) // 128 KiB
	}

	/// This is super dangerous as it doesn't enforce any assumptions of the protocol and is designed just for tests.
	#[cfg(test)]
	pub fn dangerously_new(size: u32) -> Self {
		Self(size)
	}

	pub fn size(&self) -> u32 {
		self.0
	}
}
