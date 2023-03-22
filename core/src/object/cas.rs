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
	let mut file = File::open(path).await?;
	let mut hasher = Hasher::new();
	hasher.update(&size.to_le_bytes());

	let sample_interval = if SAMPLE_COUNT * SAMPLE_SIZE > size {
		size
	} else {
		size / SAMPLE_COUNT
	};

	for i in 0..=SAMPLE_COUNT {
		let offset = if i == SAMPLE_COUNT {
			size - SAMPLE_SIZE
		} else {
			sample_interval * i
		};
		let buf = read_at(&mut file, offset, SAMPLE_SIZE).await?;
		hasher.update(&buf);
	}

	let mut id = hasher.finalize().to_hex();
	id.truncate(16);
	Ok(id.to_string())
}
