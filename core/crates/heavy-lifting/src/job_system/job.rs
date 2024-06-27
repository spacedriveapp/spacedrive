use crate::{Error, NonCriticalError, UpdateEvent};

use sd_core_sync::Manager as SyncManager;

use sd_prisma::prisma::PrismaClient;
use sd_task_system::{
	BaseTaskDispatcher, Task, TaskDispatcher, TaskHandle, TaskRemoteController, TaskSystemError,
};

use std::{
	collections::{hash_map::DefaultHasher, VecDeque},
	fmt,
	hash::{Hash, Hasher},
	marker::PhantomData,
	ops::{Deref, DerefMut},
	panic::AssertUnwindSafe,
	path::Path,
	pin::pin,
	sync::Arc,
	time::Duration,
};

use async_channel as chan;
use chrono::{DateTime, Utc};
use futures::{stream, Future, FutureExt, StreamExt};
use futures_concurrency::{
	future::{Join, TryJoin},
	stream::Merge,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use strum::{Display, EnumString};
use tokio::{
	spawn,
	sync::{oneshot, watch, Mutex},
	time::Instant,
};
use tracing::{debug, error, instrument, trace, warn, Instrument, Level};
use uuid::Uuid;

use super::{
	error::DispatcherError,
	report::{
		Report, ReportBuilder, ReportInputMetadata, ReportMetadata, ReportOutputMetadata, Status,
	},
	Command, JobId, JobSystemError, SerializableJob, SerializedTasks,
};

#[derive(
	Debug, Serialize, Deserialize, EnumString, Display, Clone, Copy, Type, Hash, PartialEq, Eq,
)]
#[strum(use_phf, serialize_all = "snake_case")]
pub enum JobName {
	Indexer,
	FileIdentifier,
	MediaProcessor,
	// TODO: Add more job names as needed
	Copy,
	Move,
	Delete,
	Erase,
	FileValidator,
}

pub enum ReturnStatus {
	Completed(JobReturn),
	Shutdown(Result<Option<Vec<u8>>, rmp_serde::encode::Error>),
	Canceled(JobReturn),
}

impl fmt::Debug for ReturnStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Completed(job_return) => f.debug_tuple("Completed").field(job_return).finish(),
			Self::Shutdown(_) => f.write_str("Shutdown(<Maybe Serialized Tasks Data>)"),
			Self::Canceled(job_return) => f.debug_tuple("Canceled").field(job_return).finish(),
		}
	}
}

pub enum ProgressUpdate {
	TaskCount(u64),
	CompletedTaskCount(u64),
	Message(String),
	Phase(String),
}

impl ProgressUpdate {
	pub fn message(message: impl Into<String>) -> Self {
		Self::Message(message.into())
	}

	pub fn phase(phase: impl Into<String>) -> Self {
		Self::Phase(phase.into())
	}
}

pub trait OuterContext: Send + Sync + Clone + 'static {
	fn id(&self) -> Uuid;
	fn db(&self) -> &Arc<PrismaClient>;
	fn sync(&self) -> &Arc<SyncManager>;
	fn invalidate_query(&self, query: &'static str);
	fn query_invalidator(&self) -> impl Fn(&'static str) + Send + Sync;
	fn report_update(&self, update: UpdateEvent);
	fn get_data_directory(&self) -> &Path;
}

pub trait JobContext<OuterCtx: OuterContext>: OuterContext {
	fn new(report: Report, ctx: OuterCtx) -> Self;
	fn progress(
		&self,
		updates: impl IntoIterator<Item = ProgressUpdate> + Send,
	) -> impl Future<Output = ()> + Send;
	fn progress_msg(&self, msg: impl Into<String>) -> impl Future<Output = ()> + Send {
		let msg = msg.into();
		async move {
			self.progress([ProgressUpdate::Message(msg)]).await;
		}
	}
	fn report(&self) -> impl Future<Output = impl Deref<Target = Report> + Send> + Send;
	fn report_mut(&self) -> impl Future<Output = impl DerefMut<Target = Report> + Send> + Send;
	fn get_outer_ctx(&self) -> OuterCtx;
}

pub trait Job: Send + Sync + Hash + 'static {
	const NAME: JobName;

	#[allow(unused_variables)]
	fn resume_tasks<OuterCtx: OuterContext>(
		&mut self,
		dispatcher: &JobTaskDispatcher,
		ctx: &impl JobContext<OuterCtx>,
		serialized_tasks: SerializedTasks,
	) -> impl Future<Output = Result<(), Error>> + Send {
		async move { Ok(()) }
	}

	fn run<OuterCtx: OuterContext>(
		self,
		dispatcher: JobTaskDispatcher,
		ctx: impl JobContext<OuterCtx>,
	) -> impl Future<Output = Result<ReturnStatus, Error>> + Send;
}

pub trait IntoJob<J, OuterCtx, JobCtx>
where
	J: Job + SerializableJob<OuterCtx>,
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
{
	fn into_job(self) -> Box<dyn DynJob<OuterCtx, JobCtx>>;
}

impl<J, OuterCtx, JobCtx> IntoJob<J, OuterCtx, JobCtx> for J
where
	J: Job + SerializableJob<OuterCtx>,
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
{
	fn into_job(self) -> Box<dyn DynJob<OuterCtx, JobCtx>> {
		let id = JobId::new_v4();

		Box::new(JobHolder {
			id,
			job: self,
			run_time: Duration::ZERO,
			report: ReportBuilder::new(id, J::NAME).build(),
			next_jobs: VecDeque::new(),
			_ctx: PhantomData,
		})
	}
}

impl<J, OuterCtx, JobCtx> IntoJob<J, OuterCtx, JobCtx> for JobEnqueuer<J, OuterCtx, JobCtx>
where
	J: Job + SerializableJob<OuterCtx>,
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
{
	fn into_job(self) -> Box<dyn DynJob<OuterCtx, JobCtx>> {
		self.build()
	}
}

#[derive(Debug)]
pub struct JobReturn {
	data: JobOutputData,
	metadata: Vec<ReportOutputMetadata>,
	non_critical_errors: Vec<NonCriticalError>,
}

impl JobReturn {
	#[must_use]
	pub fn builder() -> JobReturnBuilder {
		JobReturnBuilder {
			job_return: Self::default(),
		}
	}
}

impl Default for JobReturn {
	fn default() -> Self {
		Self {
			data: JobOutputData::Empty,
			metadata: vec![],
			non_critical_errors: vec![],
		}
	}
}

#[derive(Debug, Default)]
pub struct JobReturnBuilder {
	job_return: JobReturn,
}

impl JobReturnBuilder {
	#[must_use]
	pub const fn with_data(mut self, data: JobOutputData) -> Self {
		self.job_return.data = data;
		self
	}

	#[must_use]
	pub fn with_metadata(mut self, metadata: impl Into<Vec<ReportOutputMetadata>>) -> Self {
		self.job_return.metadata = metadata.into();
		self
	}

	#[must_use]
	pub fn with_non_critical_errors(mut self, errors: Vec<NonCriticalError>) -> Self {
		if self.job_return.non_critical_errors.is_empty() {
			self.job_return.non_critical_errors = errors;
		} else {
			self.job_return.non_critical_errors.extend(errors);
		}
		self
	}

	#[must_use]
	pub fn build(self) -> JobReturn {
		self.job_return
	}
}

#[derive(Serialize, Type)]
pub struct JobOutput {
	id: JobId,
	status: Status,
	job_name: JobName,
	data: JobOutputData,
	metadata: Vec<ReportMetadata>,
	non_critical_errors: Vec<NonCriticalError>,
}

impl JobOutput {
	#[instrument(
		skip_all,
		fields(
			name = %report.name,
			non_critical_errors_count = non_critical_errors.len(),
		)
	)]
	pub fn prepare_output_and_report(
		JobReturn {
			data,
			metadata,
			non_critical_errors,
		}: JobReturn,
		report: &mut Report,
	) -> Self {
		if non_critical_errors.is_empty() {
			report.status = Status::Completed;
			debug!("Job completed");
		} else {
			report.status = Status::CompletedWithErrors;
			report.non_critical_errors.extend(non_critical_errors);

			warn!(
				non_critical_errors = ?report.non_critical_errors,
				"Job completed with errors;",
			);
		}

		report.metadata.extend(metadata.into_iter().map(Into::into));

		report.completed_at = Some(Utc::now());

		Self {
			id: report.id,
			status: report.status,
			job_name: report.name,
			data,
			metadata: report.metadata.clone(),
			non_critical_errors: report.non_critical_errors.clone(),
		}
	}
}

#[derive(Debug, Serialize, Type)]
pub enum JobOutputData {
	Empty,
	// TODO: Add more types as needed
}

pub struct JobEnqueuer<J, OuterCtx, JobCtx>
where
	J: Job + SerializableJob<OuterCtx>,
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
{
	id: JobId,
	job: J,
	report_builder: ReportBuilder,
	next_jobs: VecDeque<Box<dyn DynJob<OuterCtx, JobCtx>>>,
	_ctx: PhantomData<OuterCtx>,
}

impl<J, OuterCtx, JobCtx> JobEnqueuer<J, OuterCtx, JobCtx>
where
	J: Job + SerializableJob<OuterCtx>,
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
{
	fn build(self) -> Box<dyn DynJob<OuterCtx, JobCtx>> {
		Box::new(JobHolder {
			id: self.id,
			job: self.job,
			run_time: Duration::ZERO,
			report: self.report_builder.build(),
			next_jobs: self.next_jobs,
			_ctx: self._ctx,
		})
	}

	pub fn new(job: J) -> Self {
		let id = JobId::new_v4();
		Self {
			id,
			job,
			report_builder: ReportBuilder::new(id, J::NAME),
			next_jobs: VecDeque::new(),
			_ctx: PhantomData,
		}
	}

	#[must_use]
	pub fn with_action(mut self, action: impl Into<String>) -> Self {
		self.report_builder = self.report_builder.with_action(action);
		self
	}

	#[must_use]
	pub fn with_parent_id(mut self, parent_id: JobId) -> Self {
		self.report_builder = self.report_builder.with_parent_id(parent_id);
		self
	}

	#[must_use]
	pub fn with_metadata(mut self, metadata: ReportInputMetadata) -> Self {
		self.report_builder = self.report_builder.with_metadata(metadata);
		self
	}

	#[must_use]
	pub fn enqueue_next(mut self, next: impl Job + SerializableJob<OuterCtx>) -> Self {
		let next_job_order = self.next_jobs.len() + 1;

		let mut child_job_builder = JobEnqueuer::new(next).with_parent_id(self.id);

		if let Some(parent_action) = &self.report_builder.action {
			child_job_builder =
				child_job_builder.with_action(format!("{parent_action}-{next_job_order}"));
		}

		self.next_jobs.push_back(child_job_builder.build());

		self
	}
}

pub struct JobHolder<J, OuterCtx, JobCtx>
where
	J: Job + SerializableJob<OuterCtx>,
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
{
	pub(super) id: JobId,
	pub(super) job: J,
	pub(super) report: Report,
	pub(super) run_time: Duration,
	pub(super) next_jobs: VecDeque<Box<dyn DynJob<OuterCtx, JobCtx>>>,
	pub(super) _ctx: PhantomData<OuterCtx>,
}

pub struct JobHandle<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> {
	pub(crate) id: JobId,
	pub(crate) start_time: Instant,
	pub(crate) run_time: Duration,
	pub(crate) is_running: bool,
	pub(crate) next_jobs: VecDeque<Box<dyn DynJob<OuterCtx, JobCtx>>>,
	pub(crate) ctx: JobCtx,
	pub(crate) commands_tx: chan::Sender<(Command, oneshot::Sender<()>)>,
}

impl<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>> JobHandle<OuterCtx, JobCtx> {
	#[instrument(skip(self, outer_ack_tx), fields(job_id = %self.id))]
	pub async fn send_command(
		&mut self,
		command: Command,
		outer_ack_tx: oneshot::Sender<Result<(), JobSystemError>>,
	) {
		trace!("JobHandle sending command");

		let (ack_tx, ack_rx) = oneshot::channel();

		let res = if self.commands_tx.send((command, ack_tx)).await.is_err() {
			warn!("Tried to send command to a job that was already completed");

			Ok(())
		} else {
			ack_rx
				.await
				.expect("inner ack channel closed before sending response to handle a job command");

			match self.execute_command(command).await {
				Ok(()) => self.command_children(command).await,
				Err(e) => Err(e),
			}
		};

		if res.is_ok() {
			match command {
				Command::Pause | Command::Cancel | Command::Shutdown => self.is_running = false,
				Command::Resume => self.is_running = true,
			}
		}

		outer_ack_tx
			.send(res)
			.unwrap_or_else(|_| panic!("ack channel closed before sending {command:?} response"));
	}

	#[instrument(skip_all, err)]
	async fn execute_command(&mut self, command: Command) -> Result<(), JobSystemError> {
		let (new_status, completed_at) = match command {
			Command::Pause => (Status::Paused, None),
			Command::Resume => (Status::Running, None),
			Command::Cancel => (Status::Canceled, Some(Utc::now())),
			Command::Shutdown => {
				// We don't need to do anything here, we will handle when the job returns its output
				return Ok(());
			}
		};

		{
			let mut report = self.ctx.report_mut().await;

			report.status = new_status;
			report.completed_at = completed_at;

			report.update(self.ctx.db()).await?;
		}

		Ok(())
	}

	#[instrument(skip_all, err)]
	async fn command_children(&mut self, command: Command) -> Result<(), JobSystemError> {
		let (new_status, completed_at) = match command {
			Command::Pause | Command::Shutdown => (Status::Paused, None),
			Command::Resume => (Status::Queued, None),
			Command::Cancel => (Status::Canceled, Some(Utc::now())),
		};

		self.next_jobs
			.iter_mut()
			.map(|dyn_job| dyn_job.report_mut())
			.map(|next_job_report| async {
				next_job_report.status = new_status;
				next_job_report.completed_at = completed_at;

				trace!(
					%next_job_report.id,
					"Parent job sent command to children job;",
				);

				next_job_report.update(self.ctx.db()).await
			})
			.collect::<Vec<_>>()
			.try_join()
			.await
			.map(|_| ())
			.map_err(Into::into)
	}

	#[instrument(
		skip(self),
		fields(job_id = %self.id),
		ret(level = Level::TRACE),
		err,
	)]
	pub async fn register_start(
		&mut self,
		start_time: DateTime<Utc>,
	) -> Result<(), JobSystemError> {
		trace!("JobHandle registering start of job");

		let Self { next_jobs, ctx, .. } = self;
		let db = ctx.db();

		let now = Utc::now();

		{
			let mut report = ctx.report_mut().await;

			report.status = Status::Running;
			if report.started_at.is_none() {
				report.started_at = Some(start_time);
			}

			// If the report doesn't have a created_at date, it's a new report
			if report.created_at.is_none() {
				report.create(db, now).await?;
			} else {
				// Otherwise it can be a job being resumed or a children job that was already been created
				report.update(db).await?;
			}
		}

		// Registering children jobs
		let res = next_jobs
			.iter_mut()
			.enumerate()
			.map(|(idx, dyn_job)| (idx, dyn_job.report_mut()))
			.map(|(idx, next_job_report)| async move {
				trace!(
					%next_job_report.id,
					"Parent job registering children;",
				);
				if next_job_report.created_at.is_none() {
					next_job_report
						.create(db, now + Duration::from_secs((idx + 1) as u64))
						.await
				} else {
					Ok(())
				}
			})
			.collect::<Vec<_>>()
			.try_join()
			.await
			.map(|_| ())
			.map_err(Into::into);

		ctx.invalidate_query("jobs.isActive");
		ctx.invalidate_query("jobs.reports");

		res
	}

	#[instrument(
		skip_all,
		fields(
			id = %self.id,

		),
		err
	)]
	pub async fn complete_job(
		&mut self,
		job_return: JobReturn,
	) -> Result<JobOutput, JobSystemError> {
		let Self { ctx, .. } = self;

		let mut report = ctx.report_mut().await;

		trace!("JobHandle completing");

		let output = JobOutput::prepare_output_and_report(job_return, &mut report);

		report.update(ctx.db()).await?;

		trace!("JobHandle completed");

		Ok(output)
	}

	#[instrument(
		skip(self),
		fields(
			id = %self.id,
		),
		err
	)]
	pub async fn failed_job(&mut self, e: &Error) -> Result<(), JobSystemError> {
		trace!("JobHandle registering failed job");

		let db = self.ctx.db();
		{
			let mut report = self.ctx.report_mut().await;

			error!(
				job_name = %report.name,
				"Job failed with a critical error;",
			);

			report.status = Status::Failed;
			report.critical_error = Some(e.to_string());
			report.completed_at = Some(Utc::now());

			report.update(db).await?;
		}

		trace!("JobHandle sending cancel command to children due to failure");

		self.command_children(Command::Cancel).await
	}

	#[instrument(
		skip(self),
		fields(
			id = %self.id,
		),
		err
	)]
	pub async fn cancel_job(
		&mut self,
		JobReturn {
			data,
			metadata,
			non_critical_errors,
		}: JobReturn,
	) -> Result<JobOutput, JobSystemError> {
		trace!("JobHandle canceling job");
		let db = self.ctx.db();

		let output = {
			let mut report = self.ctx.report_mut().await;

			debug!(
				job_name = %report.name,
				"Job canceled, we will cancel all children jobs;",
			);

			report.status = Status::Canceled;
			report.non_critical_errors.extend(non_critical_errors);
			report.metadata.extend(metadata.into_iter().map(Into::into));
			report.completed_at = Some(Utc::now());

			report.update(db).await?;

			JobOutput {
				id: report.id,
				status: report.status,
				job_name: report.name,
				data,
				metadata: report.metadata.clone(),
				non_critical_errors: report.non_critical_errors.clone(),
			}
		};

		trace!("JobHandle sending cancel command to children");

		self.command_children(Command::Cancel).await?;

		Ok(output)
	}
}

#[async_trait::async_trait]
pub trait DynJob<OuterCtx: OuterContext, JobCtx: JobContext<OuterCtx>>:
	Send + Sync + 'static
{
	fn id(&self) -> JobId;

	fn job_name(&self) -> JobName;

	fn hash(&self) -> u64;

	fn report_mut(&mut self) -> &mut Report;

	fn set_next_jobs(&mut self, next_jobs: VecDeque<Box<dyn DynJob<OuterCtx, JobCtx>>>);

	fn next_jobs(&self) -> &VecDeque<Box<dyn DynJob<OuterCtx, JobCtx>>>;

	async fn serialize(self: Box<Self>) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error>;

	fn dispatch(
		self: Box<Self>,
		base_dispatcher: BaseTaskDispatcher<Error>,
		ctx: OuterCtx,
		done_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
	) -> JobHandle<OuterCtx, JobCtx>;

	fn resume(
		self: Box<Self>,
		base_dispatcher: BaseTaskDispatcher<Error>,
		ctx: OuterCtx,
		serialized_tasks: Option<SerializedTasks>,
		done_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
	) -> JobHandle<OuterCtx, JobCtx>;
}

#[async_trait::async_trait]
impl<J, OuterCtx, JobCtx> DynJob<OuterCtx, JobCtx> for JobHolder<J, OuterCtx, JobCtx>
where
	J: Job + SerializableJob<OuterCtx>,
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
{
	fn id(&self) -> JobId {
		self.id
	}

	fn job_name(&self) -> JobName {
		J::NAME
	}

	fn hash(&self) -> u64 {
		let mut hasher = DefaultHasher::new();
		J::NAME.hash(&mut hasher);
		self.job.hash(&mut hasher);
		hasher.finish()
	}

	fn report_mut(&mut self) -> &mut Report {
		&mut self.report
	}

	fn set_next_jobs(&mut self, next_jobs: VecDeque<Box<dyn DynJob<OuterCtx, JobCtx>>>) {
		self.next_jobs = next_jobs;
	}

	fn next_jobs(&self) -> &VecDeque<Box<dyn DynJob<OuterCtx, JobCtx>>> {
		&self.next_jobs
	}

	async fn serialize(self: Box<Self>) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		self.job.serialize().await
	}

	#[instrument(skip_all, fields(id = %self.id))]
	fn dispatch(
		self: Box<Self>,
		base_dispatcher: BaseTaskDispatcher<Error>,
		ctx: OuterCtx,
		done_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
	) -> JobHandle<OuterCtx, JobCtx> {
		let (commands_tx, commands_rx) = chan::bounded(8);

		let ctx = JobCtx::new(self.report, ctx);

		trace!("Dispatching job");

		spawn({
			let id = self.id;
			let job = self.job;
			let ctx = ctx.clone();

			async move {
				if AssertUnwindSafe(to_spawn_job::<OuterCtx, _, _>(
					id,
					job,
					ctx,
					None,
					base_dispatcher,
					commands_rx,
					done_tx,
				))
				.catch_unwind()
				.await
				.is_err()
				{
					error!("job panicked");
				}
			}
		});

		JobHandle {
			id: self.id,
			start_time: Instant::now(),
			is_running: true,
			run_time: Duration::ZERO,
			next_jobs: self.next_jobs,
			ctx,
			commands_tx,
		}
	}

	#[instrument(
		skip_all,
		fields(
			id = %self.id,
			has_serialized_tasks = %serialized_tasks.is_some(),
		)
	)]
	fn resume(
		self: Box<Self>,
		base_dispatcher: BaseTaskDispatcher<Error>,
		ctx: OuterCtx,
		serialized_tasks: Option<SerializedTasks>,
		done_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
	) -> JobHandle<OuterCtx, JobCtx> {
		let (commands_tx, commands_rx) = chan::bounded(8);

		let ctx = JobCtx::new(self.report, ctx);

		trace!("Resuming job");

		spawn({
			let id = self.id;
			let job = self.job;
			let ctx = ctx.clone();

			async move {
				if AssertUnwindSafe(to_spawn_job::<OuterCtx, _, _>(
					id,
					job,
					ctx,
					serialized_tasks,
					base_dispatcher,
					commands_rx,
					done_tx,
				))
				.catch_unwind()
				.await
				.is_err()
				{
					error!("job panicked");
				}
			}
		});

		JobHandle {
			id: self.id,
			start_time: Instant::now(),
			is_running: true,
			run_time: self.run_time,
			next_jobs: self.next_jobs,
			ctx,
			commands_tx,
		}
	}
}

#[instrument(name = "job_executor", skip_all, fields(%job_id, name = %J::NAME))]
async fn to_spawn_job<OuterCtx, JobCtx, J>(
	job_id: JobId,
	mut job: J,
	ctx: JobCtx,
	existing_tasks: Option<SerializedTasks>,
	base_dispatcher: BaseTaskDispatcher<Error>,
	commands_rx: chan::Receiver<(Command, oneshot::Sender<()>)>,
	done_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
) where
	OuterCtx: OuterContext,
	JobCtx: JobContext<OuterCtx>,
	J: Job,
{
	enum StreamMessage {
		Commands((Command, oneshot::Sender<()>)),
		NewRemoteController(TaskRemoteController),
		Done(Result<ReturnStatus, Error>),
	}

	let mut remote_controllers = vec![];

	let (running_state_tx, running_state_rx) = watch::channel(JobRunningState::Running);

	let (dispatcher, remote_controllers_rx) =
		JobTaskDispatcher::new(job_id, base_dispatcher, running_state_rx);

	if let Some(existing_tasks) = existing_tasks {
		if let Err(e) = job.resume_tasks(&dispatcher, &ctx, existing_tasks).await {
			done_tx
				.send((job_id, Err(e)))
				.await
				.expect("jobs done tx closed on error at resume_tasks");

			return;
		}
	}

	let (tx, rx) = chan::bounded(1);

	spawn(
		async move {
			tx.send(
				AssertUnwindSafe(job.run::<OuterCtx>(dispatcher, ctx))
					.catch_unwind()
					.await
					.unwrap_or(Err(Error::JobSystem(JobSystemError::Panic(job_id)))),
			)
			.await
			.expect("job run channel closed");
		}
		.in_current_span(),
	);

	let commands_rx_to_close = commands_rx.clone();

	let mut msgs_stream = pin!((
		commands_rx.map(StreamMessage::Commands),
		remote_controllers_rx
			.clone()
			.map(StreamMessage::NewRemoteController),
		stream::once({
			let rx = rx.clone();
			async move { rx.recv().await.expect("job run rx closed") }
		})
		.map(StreamMessage::Done),
	)
		.merge());

	while let Some(msg) = msgs_stream.next().await {
		match msg {
			StreamMessage::NewRemoteController(remote_controller) => {
				trace!("new remote controller received");
				remote_controllers.push(remote_controller);
				trace!("added new remote controller");
			}
			StreamMessage::Commands((command, ack_tx)) => {
				// Add any possible pending remote controllers to the list
				while let Ok(remote_controller) = remote_controllers_rx.try_recv() {
					remote_controllers.push(remote_controller);
				}

				remote_controllers.retain(|controller| !controller.is_done());

				match command {
					Command::Pause => {
						trace!("Pausing job");
						running_state_tx.send_modify(|state| *state = JobRunningState::Paused);
						trace!(tasks_count = remote_controllers.len(), "pausing tasks;");

						remote_controllers
							.iter()
							.map(TaskRemoteController::pause)
							.collect::<Vec<_>>()
							.join()
							.await
							.into_iter()
							.for_each(|res| {
								if let Err(e) = res {
									assert!(matches!(e, TaskSystemError::TaskNotFound(_)));

									trace!("Tried to pause a task that was already completed");
								}
							});

						ack_tx.send(()).expect("ack channel closed");
						trace!("paused job");
					}

					Command::Resume => {
						trace!("Resuming job");
						running_state_tx.send_modify(|state| *state = JobRunningState::Running);
						trace!(tasks_count = remote_controllers.len(), "resuming tasks");

						remote_controllers
							.iter()
							.map(TaskRemoteController::resume)
							.collect::<Vec<_>>()
							.join()
							.await
							.into_iter()
							.for_each(|res| {
								if let Err(e) = res {
									assert!(matches!(e, TaskSystemError::TaskNotFound(_)));

									trace!("Tried to resume a task that was already completed");
								}
							});

						ack_tx.send(()).expect("ack channel closed");
						trace!("resumed job");
					}

					Command::Cancel => {
						trace!("Canceling job");
						running_state_tx.send_modify(|state| *state = JobRunningState::Canceled);
						trace!(tasks_count = remote_controllers.len(), "canceling tasks;");

						remote_controllers
							.iter()
							.map(TaskRemoteController::cancel)
							.collect::<Vec<_>>()
							.join()
							.await
							.into_iter()
							.for_each(|res| {
								if let Err(e) = res {
									assert!(matches!(e, TaskSystemError::TaskNotFound(_)));

									trace!("Tried to cancel a task that was already completed");
								}
							});

						trace!("canceled job");

						commands_rx_to_close.close();
						let res = rx.recv().await.expect("job run rx closed");
						ack_tx.send(()).expect("ack channel closed");
						trace!("Job cancellation done");

						return finish_job(job_id, res, remote_controllers, done_tx).await;
					}

					Command::Shutdown => {
						trace!("Shutting down job");
						running_state_tx.send_modify(|state| *state = JobRunningState::Shutdown);
						debug!(
							tasks_count = remote_controllers.len(),
							"shutting down tasks;"
						);

						commands_rx_to_close.close();
						// Just need to wait for the job to finish with the shutdown status
						let res = rx.recv().await.expect("job run rx closed");
						ack_tx.send(()).expect("ack channel closed");
						trace!("Job shutdown done");

						return finish_job(job_id, res, remote_controllers, done_tx).await;
					}
				}
			}

			StreamMessage::Done(res) => {
				trace!("Job done");
				commands_rx_to_close.close();
				return finish_job(job_id, res, remote_controllers, done_tx).await;
			}
		}
	}
}

#[instrument(skip(remote_controllers, done_tx))]
async fn finish_job(
	job_id: JobId,
	job_result: Result<ReturnStatus, Error>,
	mut remote_controllers: Vec<TaskRemoteController>,
	done_tx: chan::Sender<(JobId, Result<ReturnStatus, Error>)>,
) {
	trace!("Checking remove controllers");
	#[cfg(debug_assertions)]
	{
		// Just a sanity check to make sure we don't have any pending tasks left
		remote_controllers.retain(|controller| !controller.is_done());
		assert!(remote_controllers.is_empty());
		// Using #[cfg(debug_assertions)] to don't pay this retain cost in release builds
	}

	trace!("Sending job done message");

	done_tx
		.send((job_id, job_result))
		.await
		.expect("jobs done tx closed");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobRunningState {
	Running,
	Paused,
	Canceled,
	Shutdown,
}

impl Default for JobRunningState {
	fn default() -> Self {
		Self::Running
	}
}

#[derive(Debug, Clone)]
pub struct JobTaskDispatcher {
	job_id: JobId,
	dispatcher: BaseTaskDispatcher<Error>,
	remote_controllers_tx: chan::Sender<TaskRemoteController>,
	running_state: Arc<Mutex<watch::Receiver<JobRunningState>>>,
}

impl TaskDispatcher<Error> for JobTaskDispatcher {
	type DispatchError = DispatcherError;

	async fn dispatch_boxed(
		&self,
		boxed_task: Box<dyn Task<Error>>,
	) -> Result<TaskHandle<Error>, Self::DispatchError> {
		match self.wait_for_dispatch_approval().await {
			DispatchApproval::Canceled => Err(DispatcherError::JobCanceled(self.job_id)),
			DispatchApproval::Shutdown => Err(DispatcherError::Shutdown(vec![boxed_task])),
			DispatchApproval::Approved => {
				let handle = self.dispatcher.dispatch_boxed(boxed_task).await?;

				self.remote_controllers_tx
					.send(handle.remote_controller())
					.await
					.expect("remote controllers tx closed");

				Ok(handle)
			}
		}
	}

	async fn dispatch_many_boxed(
		&self,
		boxed_tasks: impl IntoIterator<Item = Box<dyn Task<Error>>> + Send,
	) -> Result<Vec<TaskHandle<Error>>, Self::DispatchError> {
		match self.wait_for_dispatch_approval().await {
			DispatchApproval::Canceled => Err(DispatcherError::JobCanceled(self.job_id)),
			DispatchApproval::Shutdown => {
				Err(DispatcherError::Shutdown(boxed_tasks.into_iter().collect()))
			}
			DispatchApproval::Approved => {
				let handles = self.dispatcher.dispatch_many_boxed(boxed_tasks).await?;

				handles
					.iter()
					.map(|handle| self.remote_controllers_tx.send(handle.remote_controller()))
					.collect::<Vec<_>>()
					.try_join()
					.await
					.expect("remote controllers tx closed");

				Ok(handles)
			}
		}
	}
}

enum DispatchApproval {
	Approved,
	Canceled,
	Shutdown,
}

impl JobTaskDispatcher {
	fn new(
		job_id: JobId,
		dispatcher: BaseTaskDispatcher<Error>,
		running_state_rx: watch::Receiver<JobRunningState>,
	) -> (Self, chan::Receiver<TaskRemoteController>) {
		let (remote_controllers_tx, remote_controllers_rx) = chan::unbounded();

		(
			Self {
				job_id,
				dispatcher,
				remote_controllers_tx,
				running_state: Arc::new(Mutex::new(running_state_rx)),
			},
			remote_controllers_rx,
		)
	}

	async fn wait_for_dispatch_approval(&self) -> DispatchApproval {
		{
			let mut running_state_rx = self.running_state.lock().await;

			if running_state_rx
				.has_changed()
				.expect("job running state watch channel unexpectedly closed")
			{
				trace!("waiting for job running state to change");
				running_state_rx
					.wait_for(|state| {
						matches!(
							*state,
							JobRunningState::Running
								| JobRunningState::Canceled | JobRunningState::Shutdown
						)
					})
					.await
					.expect("job running state watch channel unexpectedly closed");

				let state = { *running_state_rx.borrow() };

				match state {
					JobRunningState::Shutdown => return DispatchApproval::Shutdown,
					JobRunningState::Canceled => return DispatchApproval::Canceled,
					_ => {}
				}
			}
		}

		DispatchApproval::Approved
	}
}
