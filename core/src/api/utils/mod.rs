use std::path::Path;

use tokio::{fs, io};

mod invalidate;
mod library;

pub use invalidate::*;
pub use library::*;

/// Returns the size of the file or directory
pub async fn get_size(path: impl AsRef<Path>) -> Result<u64, io::Error> {
	let path = path.as_ref();
	let metadata = fs::metadata(path).await?;

	if metadata.is_dir() {
		let mut result = 0;
		let mut to_walk = vec![path.to_path_buf()];

		while let Some(path) = to_walk.pop() {
			let mut read_dir = fs::read_dir(&path).await?;

			while let Some(entry) = read_dir.next_entry().await? {
				let metadata = entry.metadata().await?;
				if metadata.is_dir() {
					to_walk.push(entry.path())
				} else {
					result += metadata.len()
				}
			}
		}

		Ok(result)
	} else {
		Ok(metadata.len())
	}
}
