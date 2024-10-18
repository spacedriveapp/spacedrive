use std::path::Path;

use tokio::task;

use super::super::{DeleteBehavior, FileData};

#[derive(Debug, Hash)]
pub struct MoveToTrashBehavior;

impl DeleteBehavior for MoveToTrashBehavior {
	async fn delete_all<I>(files: I) -> Result<(), ()>
	where
		I: IntoIterator<Item = FileData> + Send + 'static,
		I::IntoIter: Send + 'static,
	{
		task::spawn_blocking(|| trash::delete_all(files.into_iter().map(|x| x.full_path))).await;

		Ok(())
	}

	async fn delete(file_path: &Path) -> Result<(), ()> {
		task::spawn_blocking(|| trash::delete(file_path)).await;
		Ok(())
	}
}
