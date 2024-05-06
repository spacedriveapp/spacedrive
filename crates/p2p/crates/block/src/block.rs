use std::io::{self, ErrorKind};

use tokio::io::AsyncReadExt;

/// TODO
#[derive(Debug, PartialEq, Eq)]
pub struct Block<'a> {
	// TODO: File content, checksum, source location so it can be resent!
	pub offset: u64,
	pub size: u64,
	pub data: &'a [u8],
	// TODO: Checksum?
}

impl<'a> Block<'a> {
	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		let mut buf = Vec::new();
		buf.extend_from_slice(&self.offset.to_le_bytes());
		debug_assert_eq!(self.data.len(), self.size as usize); // TODO: Should `self.size` be inferred instead?
		buf.extend_from_slice(&self.size.to_le_bytes());
		buf.extend_from_slice(self.data);
		buf
	}

	pub async fn from_stream(
		stream: &mut (impl AsyncReadExt + Unpin),
		data_buf: &mut [u8],
	) -> Result<Block<'a>, io::Error> {
		let mut offset = [0; 8];
		stream.read_exact(&mut offset).await?;
		let offset = u64::from_le_bytes(offset);

		let mut size = [0; 8];
		stream.read_exact(&mut size).await?;
		let size = u64::from_le_bytes(size);

		// TODO: Ensure `size` is `block_size` or smaller else buffer overflow

		if size as usize > data_buf.len() {
			return Err(io::Error::new(
				ErrorKind::Other,
				"size and buffer length mismatch",
			));
		}

		stream.read_exact(&mut data_buf[..size as usize]).await?;

		Ok(Self {
			offset,
			size,
			data: &[], // TODO: This is super cringe. Data should be decoded here but lifetimes and extra allocations become a major concern.
		})
	}
}

#[cfg(test)]
mod tests {
	use std::io::Cursor;

	use super::*;

	#[tokio::test]
	async fn test_block() {
		let mut req = Block {
			offset: 420,
			size: 10, // Matches length of string on next line
			data: b"Spacedrive".as_ref(),
		};
		let bytes = req.to_bytes();
		let mut data2 = vec![0; req.data.len()];
		let req2 = Block::from_stream(&mut Cursor::new(bytes), &mut data2)
			.await
			.unwrap();
		let data = std::mem::take(&mut req.data);
		assert_eq!(req, req2);
		assert_eq!(data, data2);
	}

	#[tokio::test]
	#[should_panic] // TODO: This currently panics but long term it should have proper error handling
	async fn test_block_data_buf_overflow() {
		let mut req = Block {
			offset: 420,
			size: 10, // Matches length of string on next line
			data: b"Spacedrive".as_ref(),
		};
		let bytes = req.to_bytes();
		let mut data2 = vec![0; 5]; // Length smaller than `req.data.len()`
		let req2 = Block::from_stream(&mut Cursor::new(bytes), &mut data2)
			.await
			.unwrap();
		let data = std::mem::take(&mut req.data);
		assert_eq!(req, req2);
		assert_eq!(data, data2);
	}
}
