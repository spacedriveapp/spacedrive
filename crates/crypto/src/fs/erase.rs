use std::io::{Read, Seek, Write};

use rand::{RngCore, SeedableRng};

use crate::{primitives::BLOCK_SIZE, Result};

/// This is used for erasing a file.
///
/// It requires the file size, a stream and the amount of passes (to overwrite the entire stream with random data)
///
/// It works against `BLOCK_SIZE`.
///
/// Note, it will not be ideal on flash-based storage devices.
/// The drive will be worn down, and due to wear-levelling built into the drive's firmware no tool (short of an ATA secure erase command)
/// can guarantee a perfect erasure on solid-state drives.
///
/// This also does not factor in temporary files, caching, thumbnails, etc.
pub fn erase<RW>(stream: &mut RW, size: usize, passes: usize) -> Result<()>
where
	RW: Read + Write + Seek,
{
	let block_count = size / BLOCK_SIZE;
	let additional = size % BLOCK_SIZE;

	let mut buf = vec![0u8; BLOCK_SIZE].into_boxed_slice();
	let mut end_buf = vec![0u8; additional].into_boxed_slice();

	for _ in 0..passes {
		stream.rewind()?;
		for _ in 0..block_count {
			rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut buf);
			stream.write_all(&buf)?;
		}

		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut end_buf);
		stream.write_all(&end_buf)?;
		stream.flush()?;
	}

	stream.rewind()?;

	Ok(())
}
