use std::path::Path;

use blake3::Hasher;
use static_assertions::const_assert;
use tokio::{
	fs::{self, File},
	io::{self, AsyncReadExt, AsyncSeekExt, SeekFrom},
};

const SAMPLE_COUNT: u64 = 4;
const SAMPLE_SIZE: u64 = 1024 * 10;
const HEADER_OR_FOOTER_SIZE: u64 = 1024 * 8;

// minimum file size of 100KiB, to avoid sample hashing for small files as they can be smaller than the total sample size
const MINIMUM_FILE_SIZE: u64 = 1024 * 100;

// Asserting that nobody messed up our consts
const_assert!(
	HEADER_OR_FOOTER_SIZE + SAMPLE_COUNT * SAMPLE_SIZE + HEADER_OR_FOOTER_SIZE < MINIMUM_FILE_SIZE
);

pub async fn generate_cas_id(path: impl AsRef<Path>, size: u64) -> Result<String, io::Error> {
	let mut hasher = Hasher::new();
	hasher.update(&size.to_le_bytes());

	if size <= MINIMUM_FILE_SIZE {
		// For small files, we hash the whole file
		fs::read(path).await.map(|buf| {
			hasher.update(&buf);
		})?;
	} else {
		let mut file = File::open(path).await?;
		let mut buf = vec![0; SAMPLE_SIZE as usize].into_boxed_slice();

		// Hashing the header
		file.read_exact(&mut buf[..HEADER_OR_FOOTER_SIZE as usize])
			.await?;
		hasher.update(&buf);

		// Sample hashing the inner content of the file
		for _ in 0..SAMPLE_COUNT {
			file.seek(SeekFrom::Current(
				((size - HEADER_OR_FOOTER_SIZE * 2) / SAMPLE_COUNT) as i64,
			))
			.await?;
			file.read_exact(&mut buf).await?;
			hasher.update(&buf);
		}

		// Hashing the footer
		file.seek(SeekFrom::End(-(HEADER_OR_FOOTER_SIZE as i64)))
			.await?;
		file.read_exact(&mut buf[..HEADER_OR_FOOTER_SIZE as usize])
			.await?;
		hasher.update(&buf);
	}

	Ok(hasher.finalize().to_hex()[..16].to_string())
}
