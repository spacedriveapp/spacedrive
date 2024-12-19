mod job;
mod tasks;

use std::path::PathBuf;

pub use job::DeleteJob;

/// Delete a file or directory by moving it to trash
pub async fn move_to_trash(paths: Vec<PathBuf>, check_index: bool) -> Result<(), Error> {
	DeleteJob::new(paths, true, check_index).run().await
}

/// Permanently delete a file or directory
pub async fn remove(paths: Vec<PathBuf>, check_index: bool) -> Result<(), Error> {
	DeleteJob::new(paths, false, check_index).run().await
}
