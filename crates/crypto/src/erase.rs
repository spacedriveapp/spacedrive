use crate::{rng::CryptoRng, Error};

use std::io::{Read, Seek, Write};

use rand_core::RngCore;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// Erasing in blocks of 1MiB
const BLOCK_LEN: usize = 1_048_576;

/// This is used for erasing a stream asynchronously.
///
/// It requires the size, an input stream and the amount of passes (to overwrite the entire stream with random data).
///
/// It works against `BLOCK_LEN`.
///
/// Note, it will not be ideal on flash-based storage devices.
/// The drive will be worn down, and due to wear-levelling built into the drive's firmware no tool (short of an ATA secure erase command)
/// can guarantee a perfect erasure on solid-state drives.
///
/// This also does not factor in temporary files, caching, thumbnails, etc.
///
/// If you are dealing with files, ensure that you truncate the length to zero before removing it via the standard
/// filesystem deletion function.
pub async fn erase<RW>(stream: &mut RW, size: usize, passes: usize) -> Result<usize, Error>
where
	RW: AsyncReadExt + AsyncWriteExt + AsyncSeekExt + Unpin + Send,
{
	let mut rng = CryptoRng::new()?;

	let mut buf = vec![0u8; BLOCK_LEN].into_boxed_slice();
	let mut end_buf = vec![0u8; size % BLOCK_LEN].into_boxed_slice();

	let mut count = 0usize;
	for _ in 0..passes {
		stream.rewind().await.map_err(|e| Error::EraseIo {
			context: "Rewinding stream",
			source: e,
		})?;
		for _ in 0..(size / BLOCK_LEN) {
			rng.fill_bytes(&mut buf);
			stream.write_all(&buf).await.map_err(|e| Error::EraseIo {
				context: "Writing random bytes to stream",
				source: e,
			})?;
			count += BLOCK_LEN;
		}

		rng.fill_bytes(&mut end_buf);
		stream
			.write_all(&end_buf)
			.await
			.map_err(|e| Error::EraseIo {
				context: "Writing last block to stream",
				source: e,
			})?;
		stream.flush().await.map_err(|e| Error::EraseIo {
			context: "Flushing stream",
			source: e,
		})?;
		count += size % BLOCK_LEN;
	}

	stream.rewind().await.map_err(|e| Error::EraseIo {
		context: "Final stream rewind",
		source: e,
	})?;

	Ok(count)
}

/// This is used for erasing a stream.
///
/// It requires the size, an input stream and the amount of passes (to overwrite the entire stream with random data)
///
/// It works against `BLOCK_LEN`.
///
/// Note, it will not be ideal on flash-based storage devices.
/// The drive will be worn down, and due to wear-levelling built into the drive's firmware no tool (short of an ATA secure erase command)
/// can guarantee a perfect erasure on solid-state drives.
///
/// This also does not factor in temporary files, caching, thumbnails, etc.
///
/// If you are dealing with files, ensure that you truncate the length to zero before removing it via the standard
/// filesystem deletion function.
pub fn erase_sync<RW>(stream: &mut RW, size: usize, passes: usize) -> Result<usize, Error>
where
	RW: Read + Write + Seek,
{
	let mut rng = CryptoRng::new()?;

	let mut buf = vec![0u8; BLOCK_LEN].into_boxed_slice();
	let mut end_buf = vec![0u8; size % BLOCK_LEN].into_boxed_slice();

	let mut count = 0;
	for _ in 0..passes {
		stream.rewind().map_err(|e| Error::EraseIo {
			context: "Rewinding stream",
			source: e,
		})?;
		for _ in 0..(size / BLOCK_LEN) {
			rng.fill_bytes(&mut buf);
			stream.write_all(&buf).map_err(|e| Error::EraseIo {
				context: "Writing random bytes to stream",
				source: e,
			})?;
			count += BLOCK_LEN;
		}

		rng.fill_bytes(&mut end_buf);
		stream.write_all(&end_buf).map_err(|e| Error::EraseIo {
			context: "Writing last block to stream",
			source: e,
		})?;
		stream.flush().map_err(|e| Error::EraseIo {
			context: "Flushing stream",
			source: e,
		})?;
		count += size % BLOCK_LEN;
	}

	stream.rewind().map_err(|e| Error::EraseIo {
		context: "Final stream rewind",
		source: e,
	})?;

	Ok(count)
}

#[cfg(test)]
mod tests {
	use crate::ct::ConstantTimeEqNull;

	use std::io::Cursor;

	use super::{erase, erase_sync, BLOCK_LEN};

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_one_pass() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase_sync(&mut buffer, BLOCK_LEN, 1).unwrap();
		assert_eq!(count, BLOCK_LEN);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase_sync(&mut buffer, BLOCK_LEN, 2).unwrap();
		assert_eq!(count, BLOCK_LEN * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_5_blocks_one_pass() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase_sync(&mut buffer, BLOCK_LEN * 5, 1).unwrap();
		assert_eq!(count, BLOCK_LEN * 5);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_5_blocks_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase_sync(&mut buffer, BLOCK_LEN * 5, 2).unwrap();
		assert_eq!(count, (BLOCK_LEN * 5) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_small() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase_sync(&mut buffer, 1024, 1).unwrap();
		assert_eq!(count, 1024);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_small_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase_sync(&mut buffer, 1024, 2).unwrap();
		assert_eq!(count, 1024 * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_plus_512() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase_sync(&mut buffer, BLOCK_LEN + 512, 1).unwrap();
		assert_eq!(count, BLOCK_LEN + 512);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_plus_512_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase_sync(&mut buffer, BLOCK_LEN + 512, 2).unwrap();
		assert_eq!(count, (BLOCK_LEN + 512) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_eight_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase_sync(&mut buffer, BLOCK_LEN, 8).unwrap();
		assert_eq!(count, BLOCK_LEN * 8);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_one_pass_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase(&mut buffer, BLOCK_LEN, 1).await.unwrap();
		assert_eq!(count, BLOCK_LEN);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase(&mut buffer, BLOCK_LEN, 2).await.unwrap();
		assert_eq!(count, BLOCK_LEN * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_5_blocks_one_pass_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase(&mut buffer, BLOCK_LEN * 5, 1).await.unwrap();
		assert_eq!(count, BLOCK_LEN * 5);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_5_blocks_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase(&mut buffer, BLOCK_LEN * 5, 2).await.unwrap();
		assert_eq!(count, (BLOCK_LEN * 5) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_small_async() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase(&mut buffer, 1024, 1).await.unwrap();
		assert_eq!(count, 1024);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_small_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase(&mut buffer, 1024, 2).await.unwrap();
		assert_eq!(count, 1024 * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_plus_512_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase(&mut buffer, BLOCK_LEN + 512, 1).await.unwrap();
		assert_eq!(count, BLOCK_LEN + 512);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_plus_512_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase(&mut buffer, BLOCK_LEN + 512, 2).await.unwrap();
		assert_eq!(count, (BLOCK_LEN + 512) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_eight_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase(&mut buffer, BLOCK_LEN, 8).await.unwrap();
		assert_eq!(count, BLOCK_LEN * 8);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}
}
