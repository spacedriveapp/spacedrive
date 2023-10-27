/// The practice of dividing files into hex coded folders, often called "sharding,"
/// is mainly used to optimize file system performance. File systems can start to slow down
/// as the number of files in a directory increases. Thus, it's often beneficial to split
/// files into multiple directories to avoid this performance degradation.

/// `get_shard_hex` takes a cas_id (a hexadecimal hash) as input and returns the first
/// three characters of the hash as the directory name. Because we're using these first
/// three characters of a the hash, this will give us 4096 (16^3) possible directories,
/// named 000 to fff.
pub fn get_shard_hex(cas_id: &str) -> &str {
	// Use the first three characters of the hash as the directory name
	&cas_id[0..3]
}
