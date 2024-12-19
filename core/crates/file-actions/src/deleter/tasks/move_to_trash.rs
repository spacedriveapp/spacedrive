use sd_task_system::{check_interruption, ExecStatus, Interrupter};
use tokio::task;

use super::super::{DeleteBehavior, FileData};

#[derive(Debug, Hash)]
pub struct MoveToTrashBehavior;

impl DeleteBehavior for MoveToTrashBehavior {
	async fn delete_all<I>(files: I, interrupter: Option<&Interrupter>) -> Result<ExecStatus, ()>
	where
		I: IntoIterator<Item = FileData> + Send + 'static,
		I::IntoIter: Send + 'static,
	{
		if let Some(interrupter) = interrupter {
			check_interruption!(interrupter);
		}
		task::spawn_blocking(|| trash::delete_all(files.into_iter().map(|x| x.full_path))).await;

		Ok(ExecStatus::Done(().into()))
	}

	async fn delete(file_data: FileData) -> Result<ExecStatus, ()> {
		task::spawn_blocking(move || trash::delete(file_data.full_path)).await;
		Ok(ExecStatus::Done(().into()))
	}
}
