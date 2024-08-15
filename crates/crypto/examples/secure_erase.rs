use sd_crypto::{erase::erase_sync, rng::CryptoRng};

use std::io::{Seek, Write};

use tempfile::tempfile;

fn main() {
	let mut file = tempfile().unwrap();
	let mut rng = CryptoRng::new().unwrap();
	let data = rng.generate_vec(1048576 * 16);
	file.write_all(&data).unwrap();

	file.rewind().unwrap();

	// Erase the file (the size would normally be obtained via `fs::Metadata::len()` or similar)
	erase_sync(&mut file, 1048576 * 16, 2).unwrap();

	// Truncate the file to a length of zero
	file.set_len(0).unwrap();

	// Normally you would call `fs::remove_file()` here, however `tempfile` doesn't let us do that
	drop(file);
}
