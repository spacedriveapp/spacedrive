use std::{
	future::{pending, IntoFuture},
	time::Duration,
};

use sd_task_system::{
	ExecStatus, Interrupter, InterruptionKind, IntoAnyTaskOutput, Task, TaskId, TaskOutput,
};

use async_channel as chan;
use async_trait::async_trait;
use futures::FutureExt;
use futures_concurrency::future::Race;
use thiserror::Error;
use tokio::{
	sync::oneshot,
	time::{sleep, Instant},
};
use tracing::{error, info, instrument};

#[derive(Debug, Error)]
pub enum SampleError {
	#[error("Sample error")]
	SampleError,
}

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
impl Task<SampleError> for NeverTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
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

#[derive(Debug)]
pub struct ReadyTask {
	id: TaskId,
}

impl Default for ReadyTask {
	fn default() -> Self {
		Self {
			id: TaskId::new_v4(),
		}
	}
}

#[async_trait]
impl Task<SampleError> for ReadyTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, _interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		Ok(ExecStatus::Done(TaskOutput::Empty))
	}
}

#[derive(Debug)]
pub struct BogusTask {
	id: TaskId,
}

impl Default for BogusTask {
	fn default() -> Self {
		Self {
			id: TaskId::new_v4(),
		}
	}
}

#[async_trait]
impl Task<SampleError> for BogusTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, _interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		Err(SampleError::SampleError)
	}
}

#[derive(Debug)]
pub struct TimeTask {
	id: TaskId,
	pub duration: Duration,
	priority: bool,
	pub paused_count: u32,
}

impl TimeTask {
	pub fn new(duration: Duration, priority: bool) -> Self {
		Self {
			id: TaskId::new_v4(),
			duration,
			priority,
			paused_count: 0,
		}
	}

	pub fn with_id(id: TaskId, duration: Duration, priority: bool, paused_count: u32) -> Self {
		Self {
			id,
			duration,
			priority,
			paused_count,
		}
	}
}

#[derive(Debug)]
pub struct TimedTaskOutput {
	pub pauses_count: u32,
}

#[async_trait]
impl Task<SampleError> for TimeTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		let start = Instant::now();

		info!("Running timed task for {:#?}", self.duration);

		enum RaceOutput {
			Paused(Duration),
			Canceled,
			Completed,
		}

		let task_work_fut = async {
			sleep(self.duration).await;
			RaceOutput::Completed
		};

		let interrupt_fut = async {
			let elapsed = start.elapsed();
			match interrupter.await {
				InterruptionKind::Pause => RaceOutput::Paused(if elapsed < self.duration {
					self.duration - elapsed
				} else {
					Duration::ZERO
				}),
				InterruptionKind::Cancel => RaceOutput::Canceled,
			}
		};

		Ok(match (task_work_fut, interrupt_fut).race().await {
			RaceOutput::Completed | RaceOutput::Paused(Duration::ZERO) => ExecStatus::Done(
				TimedTaskOutput {
					pauses_count: self.paused_count,
				}
				.into_output(),
			),
			RaceOutput::Paused(remaining_duration) => {
				self.duration = remaining_duration;
				self.paused_count += 1;
				ExecStatus::Paused
			}
			RaceOutput::Canceled => ExecStatus::Canceled,
		})
	}

	fn with_priority(&self) -> bool {
		self.priority
	}
}

#[derive(Debug)]
pub struct PauseOnceTask {
	id: TaskId,
	has_paused: bool,
	began_tx: Option<oneshot::Sender<()>>,
}

impl PauseOnceTask {
	pub fn new() -> (Self, oneshot::Receiver<()>) {
		let (tx, rx) = oneshot::channel();
		(
			Self {
				id: TaskId::new_v4(),
				has_paused: false,
				began_tx: Some(tx),
			},
			rx,
		)
	}
}

#[async_trait]
impl Task<SampleError> for PauseOnceTask {
	fn id(&self) -> TaskId {
		self.id
	}

	#[instrument(skip(self, interrupter), fields(task_id = %self.id))]
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		if let Some(began_tx) = self.began_tx.take() {
			if began_tx.send(()).is_err() {
				error!("Failed to send began signal");
			}
		}

		if !self.has_paused {
			self.has_paused = true;
			info!("waiting for pause");
			match interrupter.await {
				InterruptionKind::Pause => {
					info!("Pausing PauseOnceTask <id='{}'>", self.id);
					self.has_paused = true;
					Ok(ExecStatus::Paused)
				}
				InterruptionKind::Cancel => {
					info!("Canceling PauseOnceTask <id='{}'>", self.id);
					Ok(ExecStatus::Canceled)
				}
			}
		} else {
			Ok(ExecStatus::Done(TaskOutput::Empty))
		}
	}
}

#[derive(Debug)]
pub struct BrokenTask {
	id: TaskId,
	began_tx: Option<oneshot::Sender<()>>,
}

impl BrokenTask {
	pub fn new() -> (Self, oneshot::Receiver<()>) {
		let (tx, rx) = oneshot::channel();
		(
			Self {
				id: TaskId::new_v4(),
				began_tx: Some(tx),
			},
			rx,
		)
	}
}

#[async_trait]
impl Task<SampleError> for BrokenTask {
	fn id(&self) -> TaskId {
		self.id
	}

	async fn run(&mut self, _: &Interrupter) -> Result<ExecStatus, SampleError> {
		if let Some(began_tx) = self.began_tx.take() {
			if began_tx.send(()).is_err() {
				error!("Failed to send began signal");
			}
		}

		pending().await
	}
}

#[derive(Debug)]
pub struct WaitSignalTask {
	id: TaskId,
	signal_rx: chan::Receiver<()>,
}

impl WaitSignalTask {
	pub fn new() -> (Self, chan::Sender<()>) {
		let (signal_tx, signal_rx) = chan::bounded(1);
		(
			Self {
				id: TaskId::new_v4(),
				signal_rx,
			},
			signal_tx,
		)
	}
}

#[async_trait]
impl Task<SampleError> for WaitSignalTask {
	fn id(&self) -> TaskId {
		self.id
	}

	#[instrument(skip(self, interrupter), fields(task_id = %self.id))]
	async fn run(&mut self, interrupter: &Interrupter) -> Result<ExecStatus, SampleError> {
		enum RaceOutput {
			Signal,
			Interrupt(InterruptionKind),
		}

		let race = (
			self.signal_rx.recv().map(|res| {
				res.unwrap();
				RaceOutput::Signal
			}),
			interrupter.into_future().map(RaceOutput::Interrupt),
		);

		match race.race().await {
			RaceOutput::Signal => Ok(ExecStatus::Done(TaskOutput::Empty)),
			RaceOutput::Interrupt(kind) => match kind {
				InterruptionKind::Pause => {
					info!("Paused");
					Ok(ExecStatus::Paused)
				}
				InterruptionKind::Cancel => {
					info!("Canceled");
					Ok(ExecStatus::Canceled)
				}
			},
		}
	}
}
