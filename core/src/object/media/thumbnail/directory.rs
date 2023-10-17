use crate::util::{error::FileIOError, version_manager::VersionManager};

use std::path::{Path, PathBuf};

use int_enum::IntEnum;
use tokio::fs;
use tracing::{debug, error, trace};

use super::{get_shard_hex, ThumbnailerError, THUMBNAIL_CACHE_DIR_NAME};

#[derive(IntEnum, Debug, Clone, Copy, Eq, PartialEq)]
#[repr(i32)]
enum ThumbnailVersion {
	V1 = 1,
	V2 = 2,
	Unknown = 0,
}

pub async fn init_thumbnail_dir(data_dir: impl AsRef<Path>) -> Result<PathBuf, ThumbnailerError> {
	debug!("Initializing thumbnail directory");
	let thumbnail_dir = data_dir.as_ref().join(THUMBNAIL_CACHE_DIR_NAME);

	let version_file = thumbnail_dir.join("version.txt");
	let version_manager =
		VersionManager::<ThumbnailVersion>::new(version_file.to_str().expect("Invalid path"));

	debug!("Thumbnail directory: {:?}", thumbnail_dir);

	// create all necessary directories if they don't exist
	fs::create_dir_all(&thumbnail_dir)
		.await
		.map_err(|e| FileIOError::from((&thumbnail_dir, e)))?;

	let mut current_version = match version_manager.get_version() {
		Ok(version) => version,
		Err(_) => {
			debug!("Thumbnail version file does not exist, starting fresh");
			// Version file does not exist, start fresh
			version_manager.set_version(ThumbnailVersion::V1)?;
			ThumbnailVersion::V1
		}
	};

	while current_version != ThumbnailVersion::V2 {
		match current_version {
			ThumbnailVersion::V1 => {
				let thumbnail_dir_for_task = thumbnail_dir.clone();
				// If the migration fails, it will return the error and exit the function
				move_webp_files(&thumbnail_dir_for_task).await?;
				version_manager.set_version(ThumbnailVersion::V2)?;
				current_version = ThumbnailVersion::V2;
			}
			// If the current version is not handled explicitly, break the loop or return an error.
			_ => {
				error!("Thumbnail version is not handled: {:?}", current_version);
			}
		}
	}

	Ok(thumbnail_dir)
}

/// This function moves all webp files in the thumbnail directory to their respective shard folders.
/// It is used to migrate from V1 to V2.
async fn move_webp_files(dir: &PathBuf) -> Result<(), ThumbnailerError> {
	let mut dir_entries = fs::read_dir(dir)
		.await
		.map_err(|source| FileIOError::from((dir, source)))?;
	let mut count = 0;

	while let Ok(Some(entry)) = dir_entries.next_entry().await {
		let path = entry.path();
		if path.is_file() {
			if let Some(extension) = path.extension() {
				if extension == "webp" {
					let filename = path
						.file_name()
						.expect("Missing file name")
						.to_str()
						.expect("Failed to parse UTF8"); // we know they're cas_id's, so they're valid utf8
					let shard_folder = get_shard_hex(filename);

					let new_dir = dir.join(shard_folder);
					fs::create_dir_all(&new_dir)
						.await
						.map_err(|source| FileIOError::from((new_dir.clone(), source)))?;

					let new_path = new_dir.join(filename);
					fs::rename(&path, &new_path)
						.await
						.map_err(|source| FileIOError::from((path.clone(), source)))?;
					count += 1;
				}
			}
		}
	}
	trace!(
		"Moved {} webp files to their respective shard folders.",
		count
	);
	Ok(())
}
