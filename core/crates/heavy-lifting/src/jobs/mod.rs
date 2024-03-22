use uuid::Uuid;

mod indexer;
mod job_system;

pub type JobId = Uuid;

pub use indexer::IndexerJob;
pub use job_system::{
	error::JobSystemError,
	job::{IntoJob, JobBuilder, JobOutputData},
	report::{Report as JobReport, ReportError as JobReportError},
	JobSystem,
};
