#![allow(non_upper_case_globals)]

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt};

const KiB: u32 = 1024;
const MiB: u32 = 1024 * KiB;
const GiB: u32 = 1024 * MiB;

/// defines the size of each chunk of data that is sent
///
/// We store this in an enum so it's super efficient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockSize {
	_128KiB,
	_256KiB,
	_512KiB,
	_1MiB,
	_2MiB,
	_4MiB,
	_8MiB,
	_16MiB,
}

impl BlockSize {
	/// Determine the optimal block size for a given file size
	#[must_use]
	pub fn from_file_size(size: u64) -> Self {
		// Values directly copied from https://docs.syncthing.net/specs/bep-v1.html#selection-of-block-size
		if size < 250 * u64::from(MiB) {
			return Self::_128KiB;
		} else if size < 500 * u64::from(MiB) {
			return Self::_256KiB;
		} else if size < u64::from(GiB) {
			return Self::_512KiB;
		} else if size < 2 * u64::from(GiB) {
			return Self::_1MiB;
		} else if size < 4 * u64::from(GiB) {
			return Self::_2MiB;
		} else if size < 8 * u64::from(GiB) {
			return Self::_4MiB;
		} else if size < 16 * u64::from(GiB) {
			return Self::_8MiB;
		}
		Self::_16MiB
	}

	/// Get the size of the block in bytes
	#[must_use]
	pub fn size(&self) -> u32 {
		match self {
			Self::_128KiB => 128 * KiB,
			Self::_256KiB => 256 * KiB,
			Self::_512KiB => 512 * KiB,
			Self::_1MiB => MiB,
			Self::_2MiB => 2 * MiB,
			Self::_4MiB => 4 * MiB,
			Self::_8MiB => 8 * MiB,
			Self::_16MiB => 16 * MiB,
		}
	}

	pub async fn from_stream(stream: &mut (impl AsyncRead + Unpin)) -> io::Result<Self> {
		// WARNING: Be careful modifying this cause it may break backwards/forwards-compatibility
		match stream.read_u8().await? {
			0 => Ok(Self::_128KiB),
			1 => Ok(Self::_256KiB),
			2 => Ok(Self::_512KiB),
			3 => Ok(Self::_1MiB),
			4 => Ok(Self::_2MiB),
			5 => Ok(Self::_4MiB),
			6 => Ok(Self::_8MiB),
			7 => Ok(Self::_16MiB),
			_ => Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"Invalid block size",
			)),
		}
	}

	#[must_use]
	pub fn to_bytes(&self) -> [u8; 1] {
		// WARNING: Be careful modifying this cause it may break backwards/forwards-compatibility
		[match self {
			Self::_128KiB => 0,
			Self::_256KiB => 1,
			Self::_512KiB => 2,
			Self::_1MiB => 3,
			Self::_2MiB => 4,
			Self::_4MiB => 5,
			Self::_8MiB => 6,
			Self::_16MiB => 7,
		}]
	}
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use super::*;

	#[tokio::test]
	async fn test_block_size() {
		let req = BlockSize::_128KiB;
		let bytes = req.to_bytes();
		let req2 = BlockSize::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);

		let req = BlockSize::_16MiB;
		let bytes = req.to_bytes();
		let req2 = BlockSize::from_stream(&mut Cursor::new(bytes))
			.await
			.unwrap();
		assert_eq!(req, req2);
	}
}
