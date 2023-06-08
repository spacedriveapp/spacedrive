/// The practice of dividing files into hex coded folders, often called "sharding," is mainly used to optimize file system performance. File systems can start to slow down as the number of files in a directory increases. Thus, it's often beneficial to split files into multiple directories to avoid this performance degradation.

/// `get_shard_hex` takes a cas_id (a hexadecimal hash) as input and returns the first two characters of the hash as the directory name. Because we're using the first two characters of a the hash, this will give us 256 (16*16) possible directories, named 00 to ff.
pub fn get_shard_hex(cas_id: &str) -> String {
	// Use the first two characters of the hash as the directory name
	let directory_name = &cas_id[0..2];
	directory_name.to_string()
}
