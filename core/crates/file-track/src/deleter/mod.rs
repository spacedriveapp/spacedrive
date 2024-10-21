mod job;
mod tasks;

use std::{future::Future, path::PathBuf};

use futures_concurrency::future::Join;
use sd_core_heavy_lifting::Error;
use sd_core_prisma_helpers::file_path_with_object;
use serde::{Deserialize, Serialize};

pub use job::DeleterJob;

pub type MoveToTrashJob = DeleterJob<tasks::RemoveTask<tasks::MoveToTrashBehavior>>;

pub type RemoveJob = DeleterJob<tasks::RemoveTask<tasks::RemoveBehavior>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileData {
	pub file_path: file_path_with_object::Data,
	pub full_path: PathBuf,
}

/// Specify how the [`Deleter`] job will delete a file
pub trait DeleteBehavior {
	fn delete(file: FileData) -> impl Future<Output = Result<(), ()>> + Send;

	fn delete_all<I>(files: I) -> impl Future<Output = Result<(), ()>> + Send
	where
		I: IntoIterator<Item = FileData> + Send + 'static,
		I::IntoIter: Send,
	{
		async {
			files
				.into_iter()
				.map(Self::delete)
				.collect::<Vec<_>>()
				.join()
				.await;
			Ok(())
		}
	}
}
