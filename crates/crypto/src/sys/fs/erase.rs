use crate::{primitives::BLOCK_LEN, rng::CryptoRng, Result};
use std::io::{Read, Seek, Write};

use rand_core::RngCore;
#[cfg(feature = "tokio")]
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

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
pub fn erase<RW>(stream: &mut RW, size: usize, passes: usize) -> Result<usize>
where
	RW: Read + Write + Seek,
{
	let mut count = 0usize;

	let mut buf = vec![0u8; BLOCK_LEN].into_boxed_slice();
	let mut end_buf = vec![0u8; size % BLOCK_LEN].into_boxed_slice();

	for _ in 0..passes {
		stream.rewind()?;
		for _ in 0..(size / BLOCK_LEN) {
			CryptoRng::new().fill_bytes(&mut buf);
			stream.write_all(&buf)?;
			count += BLOCK_LEN;
		}

		CryptoRng::new().fill_bytes(&mut end_buf);
		stream.write_all(&end_buf)?;
		stream.flush()?;
		count += size % BLOCK_LEN;
	}

	stream.rewind()?;

	Ok(count)
}

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
#[cfg(feature = "tokio")]
pub async fn erase_async<RW>(stream: &mut RW, size: usize, passes: usize) -> Result<usize>
where
	RW: AsyncReadExt + AsyncWriteExt + AsyncSeekExt + Unpin + Send,
{
	let mut count = 0usize;

	let mut buf = vec![0u8; BLOCK_LEN].into_boxed_slice();
	let mut end_buf = vec![0u8; size % BLOCK_LEN].into_boxed_slice();

	for _ in 0..passes {
		stream.rewind().await?;
		for _ in 0..(size / BLOCK_LEN) {
			CryptoRng::new().fill_bytes(&mut buf);
			stream.write_all(&buf).await?;
			count += BLOCK_LEN;
		}

		CryptoRng::new().fill_bytes(&mut end_buf);
		stream.write_all(&end_buf).await?;
		stream.flush().await?;
		count += size % BLOCK_LEN;
	}

	stream.rewind().await?;

	Ok(count)
}

#[cfg(test)]
mod tests {
	use crate::{ct::ConstantTimeEqNull, primitives::BLOCK_LEN};
	use std::io::Cursor;

	use super::erase;

	#[cfg(feature = "tokio")]
	use super::erase_async;

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_one_pass() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase(&mut buffer, BLOCK_LEN, 1).unwrap();
		assert_eq!(count, BLOCK_LEN);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase(&mut buffer, BLOCK_LEN, 2).unwrap();
		assert_eq!(count, BLOCK_LEN * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_5_blocks_one_pass() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase(&mut buffer, BLOCK_LEN * 5, 1).unwrap();
		assert_eq!(count, BLOCK_LEN * 5);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_5_blocks_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase(&mut buffer, BLOCK_LEN * 5, 2).unwrap();
		assert_eq!(count, (BLOCK_LEN * 5) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_small() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase(&mut buffer, 1024, 1).unwrap();
		assert_eq!(count, 1024);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_small_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase(&mut buffer, 1024, 2).unwrap();
		assert_eq!(count, 1024 * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_plus_512() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase(&mut buffer, BLOCK_LEN + 512, 1).unwrap();
		assert_eq!(count, BLOCK_LEN + 512);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_plus_512_two_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase(&mut buffer, BLOCK_LEN + 512, 2).unwrap();
		assert_eq!(count, (BLOCK_LEN + 512) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[test]
	#[cfg_attr(miri, ignore)]
	fn erase_block_eight_passes() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase(&mut buffer, BLOCK_LEN, 8).unwrap();
		assert_eq!(count, BLOCK_LEN * 8);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_one_pass_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase_async(&mut buffer, BLOCK_LEN, 1).await.unwrap();
		assert_eq!(count, BLOCK_LEN);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase_async(&mut buffer, BLOCK_LEN, 2).await.unwrap();
		assert_eq!(count, BLOCK_LEN * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_5_blocks_one_pass_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase_async(&mut buffer, BLOCK_LEN * 5, 1).await.unwrap();
		assert_eq!(count, BLOCK_LEN * 5);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_5_blocks_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN * 5]);
		let count = erase_async(&mut buffer, BLOCK_LEN * 5, 2).await.unwrap();
		assert_eq!(count, (BLOCK_LEN * 5) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_small_async() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase_async(&mut buffer, 1024, 1).await.unwrap();
		assert_eq!(count, 1024);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_small_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; 1024]);
		let count = erase_async(&mut buffer, 1024, 2).await.unwrap();
		assert_eq!(count, 1024 * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_plus_512_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase_async(&mut buffer, BLOCK_LEN + 512, 1).await.unwrap();
		assert_eq!(count, BLOCK_LEN + 512);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_plus_512_two_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN + 512]);
		let count = erase_async(&mut buffer, BLOCK_LEN + 512, 2).await.unwrap();
		assert_eq!(count, (BLOCK_LEN + 512) * 2);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}

	#[tokio::test]
	#[cfg(feature = "tokio")]
	#[cfg_attr(miri, ignore)]
	async fn erase_block_eight_passes_async() {
		let mut buffer = Cursor::new(vec![0u8; BLOCK_LEN]);
		let count = erase_async(&mut buffer, BLOCK_LEN, 8).await.unwrap();
		assert_eq!(count, BLOCK_LEN * 8);
		assert_eq!(buffer.position(), 0);
		assert!(bool::from(buffer.into_inner().ct_ne_null()));
	}
}
