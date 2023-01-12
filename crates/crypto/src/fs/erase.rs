use std::io::{Read, Seek, Write};

use rand::{RngCore, SeedableRng};

use crate::{primitives::BLOCK_SIZE, Result};

pub fn erase<RW>(stream: &mut RW, size: usize, passes: usize) -> Result<()>
where
	RW: Read + Write + Seek,
{
	let block_count = size / BLOCK_SIZE;
	let additional = size % BLOCK_SIZE;

	let mut buf = vec![0u8; block_count];
	let mut end_buf = vec![0u8; additional];

	for _ in 0..passes {
		stream.rewind()?;
		for _ in 0..block_count {
			rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut buf);
			stream.write(&buf)?;
		}

		rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut end_buf);
		stream.write(&end_buf)?;
		stream.flush()?;
	}

	Ok(())
}
