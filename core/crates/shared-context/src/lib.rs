use std::{
	ops::{Deref, DerefMut},
	path::Path,
	sync::Arc,
};

use chrono::{DateTime, Utc};
use sd_core_shared_types::jobs::progress::JobProgressEvent;
use sd_prisma::prisma::PrismaClient;
use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Represents an update event that can be sent to the job system
#[derive(Debug, Clone)]
pub enum UpdateEvent {
	/// Progress update for a job
	Progress(JobProgressEvent),
	/// A query that needs to be invalidated
	InvalidateQuery(&'static str),
}

/// Represents a progress update for a job
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
	/// The task progress
	TaskCount(u64),
	/// A message to display
	Message(String),
	/// The current phase of the job
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

/// The outer context trait that provides access to shared resources
pub trait OuterContext: Clone + Send + Sync + 'static {
	/// Get the unique identifier for this context
	fn id(&self) -> Uuid;

	/// Get the database client
	fn db(&self) -> &Arc<PrismaClient>;

	/// Get the sync manager
	fn sync(&self) -> &Arc<impl Send + Sync>;

	/// Invalidate a query
	fn invalidate_query(&self, query: &'static str);

	/// Get a function that can invalidate queries
	fn query_invalidator(&self) -> impl Fn(&'static str) + Send + Sync;

	/// Report an update
	fn report_update(&self, update: UpdateEvent);

	/// Get the data directory
	fn get_data_directory(&self) -> &Path;
}

/// The job context trait that provides access to job-specific resources
pub trait JobContext<OuterCtx: OuterContext>: Clone + Send + Sync + 'static {
	/// Create a new job context
	fn new(report: Report, outer_ctx: OuterCtx) -> Self;

	/// Send progress updates
	fn progress(&self, updates: impl IntoIterator<Item = ProgressUpdate> + Send);

	/// Get the report
	fn report(&self) -> impl Deref<Target = Report>;

	/// Get the report mutably
	fn report_mut(&self) -> impl DerefMut<Target = Report>;

	/// Get the outer context
	fn get_outer_ctx(&self) -> OuterCtx;
}

/// A report for a job
#[derive(Debug, Clone)]
pub struct Report {
	/// The current status of the job
	pub status: Status,
	/// When the job started
	pub started_at: Option<DateTime<Utc>>,
	/// When the job completed
	pub completed_at: Option<DateTime<Utc>>,
	/// The current task count
	pub task_count: Arc<RwLock<u64>>,
	/// The completed task count
	pub completed_task_count: Arc<RwLock<u64>>,
	/// The current message
	pub message: Arc<RwLock<Option<String>>>,
	/// The current phase
	pub phase: Arc<RwLock<Option<String>>>,
	/// The estimated completion time
	pub estimated_completion: Arc<RwLock<DateTime<Utc>>>,
	/// Additional info about the job
	pub info: Arc<RwLock<Option<Value>>>,
}

impl Report {
	pub fn new() -> Self {
		Self {
			status: Status::Queued,
			started_at: None,
			completed_at: None,
			task_count: Arc::new(RwLock::new(0)),
			completed_task_count: Arc::new(RwLock::new(0)),
			message: Arc::new(RwLock::new(None)),
			phase: Arc::new(RwLock::new(None)),
			estimated_completion: Arc::new(RwLock::new(Utc::now())),
			info: Arc::new(RwLock::new(None)),
		}
	}
}

/// The status of a job
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
	/// The job is queued
	Queued,
	/// The job is running
	Running,
	/// The job is paused
	Paused,
	/// The job completed successfully
	Completed,
	/// The job failed
	Failed,
	/// The job was cancelled
	Cancelled,
}
