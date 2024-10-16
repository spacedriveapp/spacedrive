mod move_to_trash;
mod remove;

use std::{
	marker::PhantomData,
	sync::{atomic::AtomicU64, Arc},
};

pub use move_to_trash::MoveToTrashBehavior;
pub use remove::RemoveBehavior;
use sd_task_system::{check_interruption, ExecStatus, Interrupter, Task, TaskId, TaskOutput};

use super::DeleteBehavior;
use super::FileData;

pub struct RemoveTask<B> {
	id: TaskId,
	files: Vec<FileData>,
	counter: Arc<AtomicU64>,
	behavior: PhantomData<fn(B) -> B>,
}

impl<B: DeleteBehavior> RemoveTask<B> {
	pub fn new(files: Vec<FileData>, counter: Arc<AtomicU64>) -> Self {
		Self {
			id: TaskId::new_v4(),
			files,
			counter,
			behavior: PhantomData,
		}
	}
}

#[async_trait::async_trait]
impl<B: DeleteBehavior + Send + 'static> Task<super::Error> for RemoveTask<B> {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, super::Error> {
		tracing::debug!(id=%self.id, "running remove task");

		check_interruption!(interrupter);

		let size = self.files.len();

		// TODO(matheus-consoli): unnecessary clone
		B::delete_all(self.files.clone()).await;

		self.counter
			.fetch_add(size as _, std::sync::atomic::Ordering::AcqRel);

		Ok(ExecStatus::Done(TaskOutput::Empty))
	}
}
