use async_trait::async_trait;
use futures_concurrency::future::FutureGroup;
use lending_stream::{LendingStream, StreamExt};
use sd_task_system::{
	BaseTaskDispatcher, ExecStatus, Interrupter, IntoAnyTaskOutput, Task, TaskDispatcher,
	TaskHandle, TaskId, TaskOutput, TaskStatus,
};
use tracing::trace;

use super::tasks::SampleError;

#[derive(Debug)]
pub struct SampleJob {
	total_steps: u32,
	task_dispatcher: BaseTaskDispatcher<SampleError>,
}

impl SampleJob {
	pub fn new(total_steps: u32, task_dispatcher: BaseTaskDispatcher<SampleError>) -> Self {
		Self {
			total_steps,
			task_dispatcher,
		}
	}

	pub async fn run(self) -> Result<(), SampleError> {
		let Self {
			total_steps,
			task_dispatcher,
		} = self;

		let initial_steps = (0..task_dispatcher.workers_count())
			.map(|_| SampleJobTask {
				id: TaskId::new_v4(),
				expected_children: total_steps - 1,
				task_dispatcher: task_dispatcher.clone(),
			})
			.collect::<Vec<_>>();

		let mut group = FutureGroup::from_iter(
			task_dispatcher
				.dispatch_many(initial_steps)
				.await
				.unwrap()
				.into_iter(),
		)
		.lend_mut();

		while let Some((group, res)) = group.next().await {
			match res.unwrap() {
				TaskStatus::Done((_task_id, TaskOutput::Out(out))) => {
					group.insert(
						out.downcast::<Output>()
							.expect("we know the output type")
							.children_handle,
					);
					trace!("Received more tasks to wait for ({} left)", group.len());
				}
				TaskStatus::Done((_task_id, TaskOutput::Empty)) => {
					trace!(
						"Step done, waiting for all children to finish ({} left)",
						group.len()
					);
				}

				TaskStatus::Canceled => {
					trace!("Task was canceled");
				}
				TaskStatus::ForcedAbortion => {
					trace!("Aborted")
				}
				TaskStatus::Shutdown(task) => {
					trace!("Task was shutdown: {:?}", task);
				}
				TaskStatus::Error(e) => return Err(e),
			}
		}

		Ok(())
	}
}

#[derive(Debug)]
struct SampleJobTask {
	id: TaskId,
	expected_children: u32,
	task_dispatcher: BaseTaskDispatcher<SampleError>,
}

#[derive(Debug)]
struct Output {
	children_handle: TaskHandle<SampleError>,
}

#[async_trait]
impl Task<SampleError> for SampleJobTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, _interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		if self.expected_children > 0 {
			Ok(ExecStatus::Done(
				Output {
					children_handle: self
						.task_dispatcher
						.dispatch(SampleJobTask {
							id: TaskId::new_v4(),
							expected_children: self.expected_children - 1,
							task_dispatcher: self.task_dispatcher.clone(),
						})
						.await
						.unwrap(),
				}
				.into_output(),
			))
		} else {
			Ok(ExecStatus::Done(TaskOutput::Empty))
		}
	}
}
