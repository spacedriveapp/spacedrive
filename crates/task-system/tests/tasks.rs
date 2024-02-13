use sd_task_system::{ExecStatus, Interrupter, InterruptionKind, Task, TaskId, TaskSystemError};

use async_trait::async_trait;
use tracing::info;

#[derive(Debug)]
pub struct NeverTask {
	id: TaskId,
}

impl Default for NeverTask {
	fn default() -> Self {
		Self {
			id: TaskId::new_v4(),
		}
	}
}

#[async_trait]
impl Task for NeverTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, TaskSystemError> {
		match interrupter.await {
			InterruptionKind::Pause => {
				info!("Pausing NeverTask <id='{}'>", self.id);
				Ok(ExecStatus::Paused)
			}
			InterruptionKind::Cancel => {
				info!("Canceling NeverTask <id='{}'>", self.id);
				Ok(ExecStatus::Canceled)
			}
		}
	}
}
