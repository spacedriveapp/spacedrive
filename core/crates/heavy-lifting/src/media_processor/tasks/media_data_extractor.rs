use crate::Error;

use sd_task_system::{ExecStatus, Interrupter, Task, TaskId};

#[derive(Debug)]
pub struct MediaDataExtractor {
	id: TaskId,
}

impl MediaDataExtractor {
	pub fn new() -> Self {
		Self {
			id: TaskId::new_v4(),
		}
	}
}

#[async_trait::async_trait]
impl Task<Error> for MediaDataExtractor {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, Error> {
		Ok(ExecStatus::Canceled)
	}
}
