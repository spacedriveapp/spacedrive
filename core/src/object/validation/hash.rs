use blake3::Hasher;
use std::path::Path;
use tokio::{
	fs::File,
	io::{self, AsyncReadExt},
};

const BLOCK_LEN: usize = 1048576;

pub async fn file_checksum(path: impl AsRef<Path>) -> Result<String, io::Error> {
	let mut reader = File::open(path).await?;
	let mut context = Hasher::new();
	let mut buffer = vec![0; BLOCK_LEN].into_boxed_slice();
	loop {
		let read_count = reader.read(&mut buffer).await?;
		context.update(&buffer[..read_count]);
		if read_count != BLOCK_LEN {
			break;
		}
	}
	let hex = context.finalize().to_hex();

	Ok(hex.to_string())
}
