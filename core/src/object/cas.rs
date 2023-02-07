use blake3::Hasher;
use std::path::Path;
use tokio::{
	fs::File,
	io::{self, AsyncReadExt, AsyncSeekExt, SeekFrom},
};

static SAMPLE_COUNT: u64 = 4;
static SAMPLE_SIZE: u64 = 10000;

async fn read_at(file: &mut File, offset: u64, size: u64) -> Result<Vec<u8>, io::Error> {
	let mut buf = vec![0u8; size as usize];

	file.seek(SeekFrom::Start(offset)).await?;
	file.read_exact(&mut buf).await?;

	Ok(buf)
}

pub async fn generate_cas_id(path: impl AsRef<Path>, size: u64) -> Result<String, io::Error> {
	// open file reference
	let mut file = File::open(path).await?;

	let mut hasher = Hasher::new();

	// include the file size in the checksum
	hasher.update(&size.to_le_bytes());

	// if size is small enough, just read the whole thing

	if SAMPLE_COUNT * SAMPLE_SIZE > size {
		let buf = read_at(&mut file, 0, size).await?;
		hasher.update(&buf);
	} else {
		// loop over samples
		for i in 0..SAMPLE_COUNT {
			let buf = read_at(&mut file, (size / SAMPLE_COUNT) * i, SAMPLE_SIZE).await?;
			hasher.update(&buf);
		}
		// sample end of file
		let buf = read_at(&mut file, size - SAMPLE_SIZE, SAMPLE_SIZE).await?;
		hasher.update(&buf);
	}

	let hex = hasher.finalize().to_hex();
	let mut id = hex.to_string();
	id.truncate(16);
	Ok(id)
}
