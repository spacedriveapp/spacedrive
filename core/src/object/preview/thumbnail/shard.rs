use blake3::Hasher;
use hex::encode;

/// The practice of dividing files into hex coded folders, often called "sharding," is mainly used to optimize file system performance. File systems can start to slow down as the number of files in a directory increases. Thus, it's often beneficial to split files into multiple directories to avoid this performance degradation.

/// `calc_shard_hex` takes a filename as input, computes a SHA256 hash of the filename, and returns the first two characters of the hash as the directory name. Because we're using the first two characters of a blake3 hash, this will give us 256 (16*16) possible directories, named 00 to ff.
pub fn calc_shard_hex(filename: &str) -> String {
	let mut hasher = Hasher::new();

	hasher.update(filename.as_bytes());

	let result = hasher.finalize();
	let hex_result = encode(result.as_bytes());

	// Use the first two characters of the hash as the directory name
	let directory_name = &hex_result[0..2];
	directory_name.to_string()
}
