use std::path::PathBuf;
use tokio::fs; // For async file operations

use crate::{
	invalidate_query, library::Library, object::fs::FileSystemJobsError, prisma::location,
};

pub async fn create_folder(
	location_id: Option<i32>,
	path: PathBuf,
	name: Option<&str>,
	library: &Library,
) -> Result<(), FileSystemJobsError> {
	// If location_id is provided, query the database to get the location
	if let Some(id) = location_id {
		library
			.db
			.location()
			.find_unique(location::id::equals(id))
			.exec()
			.await
			.map_err(|e| FileSystemJobsError::Database(e))?;
	}

	name.unwrap_or("Untitled Folder");

	let path_clone = path.clone();

	match fs::metadata(&path).await {
		Ok(metadata) if metadata.is_dir() => Ok(()),
		Ok(_) => Err(FileSystemJobsError::WouldOverwrite(path.into_boxed_path())),
		Err(_) => {
			fs::create_dir_all(path_clone)
				.await
				.map_err(|e| FileSystemJobsError::IO(e))?;

			// Invalidate search query only if a folder is created
			// TODO: Tell indexer to index the new folder
			if location_id.is_some() {
				invalidate_query!(library, "search.objects");
				invalidate_query!(library, "search.paths");
			}

			Ok(())
		}
	}?;

	Ok(())
}
