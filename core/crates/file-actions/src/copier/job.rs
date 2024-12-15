use std::{
	collections::VecDeque,
	path::{Path, PathBuf},
	sync::Arc,
};

use sd_prisma::prisma::{file_path, location};
use sd_utils::error::FileIOError;

use heavy_lifting::{
	job::{Job, JobContext, JobError},
	report::{ReportInputMetadata, ReportOutputMetadata},
	task::{Task, TaskId},
};

use super::tasks::{CopyTask, CreateDirsTask};

const MAX_TOTAL_SIZE_PER_STEP: u64 = 1024 * 1024 * 800; // 800MB
const MAX_FILES_PER_STEP: usize = 20;

#[derive(Debug)]
pub struct CopyJob {
	sources: Vec<PathBuf>,
	target_dir: PathBuf,
}

impl CopyJob {
	pub fn new(sources: Vec<PathBuf>, target_dir: impl Into<PathBuf>) -> Self {
		Self {
			sources,
			target_dir: target_dir.into(),
		}
	}

	async fn create_directory_tasks(
		&self,
		_ctx: &impl JobContext,
	) -> Result<Vec<Box<dyn Task<Error = Error>>>, JobError> {
		let mut tasks: Vec<Box<dyn Task<Error = Error>>> = Vec::new();

		// Create target directory if it doesn't exist
		if !self.target_dir.exists() {
			tasks.push(Box::new(CreateDirsTask::new(
				self.target_dir.clone(),
				self.target_dir.clone(),
			)));
		}

		// Create subdirectories for each source directory
		for source in &self.sources {
			let target_path = self.target_dir.join(source.file_name().unwrap());
			if source.is_dir() {
				tasks.push(Box::new(CreateDirsTask::new(source.clone(), target_path)));
			}
		}

		Ok(tasks)
	}

	async fn create_copy_tasks(
		&self,
		_ctx: &impl JobContext,
	) -> Result<Vec<Box<dyn Task<Error = Error>>>, JobError> {
		let mut tasks: Vec<Box<dyn Task<Error = Error>>> = Vec::new();
		let mut current_batch = Vec::new();
		let mut current_batch_size = 0;

		for source in &self.sources {
			if source.is_file() {
				let file_size = source.metadata().map_err(|e| JobError::IO(e.into()))?.len();

				// If adding this file would exceed our batch limits, create a new task
				if current_batch.len() >= MAX_FILES_PER_STEP
					|| current_batch_size + file_size > MAX_TOTAL_SIZE_PER_STEP
				{
					if !current_batch.is_empty() {
						tasks.push(Box::new(CopyTask::new(current_batch.clone())));
						current_batch.clear();
						current_batch_size = 0;
					}
				}

				current_batch.push(source.clone());
				current_batch_size += file_size;
			}
		}

		// Push any remaining files
		if !current_batch.is_empty() {
			tasks.push(Box::new(CopyTask::new(current_batch)));
		}

		Ok(tasks)
	}
}

#[async_trait::async_trait]
impl Task for CopyJob {
	fn name(&self) -> &'static str {
		"copy"
	}

	fn metadata(&self) -> ReportInputMetadata {
		ReportInputMetadata::Copier {
			location_id: None,
			sources: self.sources.clone(),
			target_dir: self.target_dir.clone(),
		}
	}

	async fn run(
		&self,
		ctx: &impl JobContext,
	) -> Result<Vec<Box<dyn Task<Error = Error>>>, JobError> {
		// First create all necessary directories
		let mut tasks = self.create_directory_tasks(ctx).await?;

		// Then create copy tasks for files
		tasks.extend(self.create_copy_tasks(ctx).await?);

		Ok(tasks)
	}
}
