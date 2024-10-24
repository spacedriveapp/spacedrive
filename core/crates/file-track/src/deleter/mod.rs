mod job;
mod tasks;

use std::{future::Future, path::PathBuf};

use futures::{stream::FuturesUnordered, StreamExt};
use sd_core_heavy_lifting::Error;
use sd_core_prisma_helpers::file_path_with_object;
use sd_task_system::{check_interruption, ExecStatus, Interrupter};
use serde::{Deserialize, Serialize};

pub use job::DeleterJob;

pub type MoveToTrashJob = DeleterJob<tasks::MoveToTrash>;

pub type RemoveJob = DeleterJob<tasks::Remove>;

// TODO(matheus-consoli): remove
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileData {
	pub file_path: file_path_with_object::Data,
	pub full_path: PathBuf,
}

/// Specify how the [`Deleter`] job will delete a file
pub trait DeleteBehavior {
	fn delete(file: FileData) -> impl Future<Output = Result<ExecStatus, ()>> + Send;

	fn delete_all<I>(
		files: I,
		interrupter: Option<&Interrupter>,
	) -> impl Future<Output = Result<ExecStatus, ()>> + Send
	where
		I: IntoIterator<Item = FileData> + Send + 'static,
		I::IntoIter: Send,
	{
		async move {
			let v = files
				.into_iter()
				.map(|file| async move {
					if let Some(interrupter) = interrupter {
						check_interruption!(interrupter);
					}
					Self::delete(file).await
				})
				.collect::<Vec<_>>();
			let mut f = FuturesUnordered::from_iter(v);
			while let Some(x) = f.next().await {
				match x {
					Ok(a @ (ExecStatus::Canceled | ExecStatus::Paused)) => {
						return Ok(a);
					}
					Err(_) => return Err(()),
					Ok(_) => {}
				}
			}
			Ok(ExecStatus::Done(().into()))
		}
	}
}
