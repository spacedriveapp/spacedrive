use super::{
	batch::{batch_copy_files, collect_copy_entries},
	behaviors::{CopyBehavior, FastCopyBehavior, StreamCopyBehavior},
	conflict::resolve_name_conflicts,
};
use crate::copier::progress::CopyProgress;
use async_trait::async_trait;
use sd_core_core_errors::Error;
use sd_core_job_system::{
	job_system::job::{JobContext, JobError},
	task::{Task, TaskId, TaskStatus},
	OuterContext, ProgressUpdate,
};
use sd_core_library_sync::SyncManager;
use sd_core_shared_types::sd_path::SdPath;
use sd_prisma::prisma::PrismaClient;
use sd_task_system::SerializableTask;
use serde::{Deserialize, Serialize};
use std::{
	fmt,
	path::{Path, PathBuf},
	sync::Arc,
	time::{Duration, Instant},
};
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct CopyTask {
	id: TaskId,
	source: SdPath,
	target: SdPath,
	progress: CopyProgress,
}

impl CopyTask {
	pub async fn new(source: SdPath, target: SdPath) -> Result<Self, JobError> {
		Ok(Self {
			id: Uuid::new_v4(),
			source,
			target,
			progress: CopyProgress::default(),
		})
	}

	async fn find_available_name(path: impl AsRef<Path>) -> Result<PathBuf, JobError> {
		let path = path.as_ref();

		if !fs::try_exists(path)
			.await
			.map_err(|e| JobError::IO(e.into()))?
		{
			return Ok(path.to_owned());
		}

		let file_stem = path
			.file_stem()
			.and_then(|s| s.to_str())
			.ok_or_else(|| JobError::InvalidInput("File has no valid stem".into()))?;

		let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");

		let parent = path
			.parent()
			.ok_or_else(|| JobError::InvalidInput("File has no parent directory".into()))?;

		for i in 1.. {
			let new_name = if extension.is_empty() {
				format!("{} ({})", file_stem, i)
			} else {
				format!("{} ({}).{}", file_stem, i, extension)
			};

			let new_path = parent.join(new_name);
			if !fs::try_exists(&new_path)
				.await
				.map_err(|e| JobError::IO(e.into()))?
			{
				return Ok(new_path);
			}
		}

		unreachable!()
	}

	async fn handle_error<OuterCtx: OuterContext>(
		&mut self,
		error: JobError,
		ctx: &impl JobContext<OuterCtx>,
	) -> Result<TaskStatus, JobError> {
		// If we have a current file, we can try to resume from there
		if let Some((source, target)) = &self.current_file {
			// Clean up the partially copied file
			if fs::try_exists(target)
				.await
				.map_err(|e| JobError::IO(e.into()))?
			{
				fs::remove_file(target)
					.await
					.map_err(|e| JobError::IO(e.into()))?;
			}

			// Return a shutdown status with our current state
			Ok(TaskStatus::Shutdown(Box::new(self.clone())))
		} else {
			Err(error)
		}
	}

	async fn run<OuterCtx: OuterContext>(
		&mut self,
		ctx: &impl JobContext<OuterCtx>,
	) -> Result<TaskStatus, JobError> {
		// Validate paths exist and are accessible
		let db = ctx.db();
		self.source
			.validate(db)
			.await
			.map_err(|e| JobError::IO(e.to_string().into()))?;
		self.target
			.validate(db)
			.await
			.map_err(|e| JobError::IO(e.to_string().into()))?;

		// Get resolved paths
		let source_path = self
			.source
			.resolve(db)
			.await
			.map_err(|e| JobError::IO(e.to_string().into()))?;
		let target_path = self
			.target
			.resolve(db)
			.await
			.map_err(|e| JobError::IO(e.to_string().into()))?;

		// If source and target are on different devices, use p2p transfer
		if self.source.device() != self.target.device() {
			// TODO: Implement p2p file transfer
			return Err(JobError::IO(
				"Cross-device file transfer not yet implemented".into(),
			));
		}

		// Same device, use regular file copy
		// Collect all files and directories
		let (files, dirs) = collect_copy_entries(&source_path, &target_path).await?;

		// Create all necessary directories
		for (_, dir) in dirs {
			fs::create_dir_all(&dir)
				.await
				.map_err(|e| JobError::IO(e.into()))?;
		}

		// Resolve any name conflicts
		let files = resolve_name_conflicts(files).await?;

		// Batch the files for optimal copying
		let batches = batch_copy_files(files).await?;

		let total_files = batches
			.iter()
			.map(|batch| batch.sources.len())
			.sum::<usize>() as u64;
		let total_bytes = batches.iter().map(|batch| batch.total_size).sum::<u64>();

		ctx.progress(vec![ProgressUpdate::Stats(serde_json::to_value(
			CopyProgress::Started {
				total_files,
				total_bytes,
			},
		)?)])
		.await;

		let mut files_copied = 0;
		let mut bytes_copied = 0;
		let start = Instant::now();

		for batch in batches {
			for (source, target) in batch.sources.into_iter().zip(batch.targets.into_iter()) {
				let target = Self::find_available_name(target).await?;

				let file_name = source
					.file_name()
					.unwrap_or_default()
					.to_string_lossy()
					.to_string();

				ctx.progress(vec![ProgressUpdate::Stats(serde_json::to_value(
					CopyProgress::File {
						name: file_name.clone(),
						current_file: (files_copied + 1),
						total_files,
						bytes: batch.total_size,
						source: source.clone(),
						target: target.clone(),
					},
				)?)])
				.await;

				let behavior = determine_behavior(&source, &target);
				match behavior.copy_file(&source, &target, ctx).await {
					Ok(()) => {
						files_copied += 1;
						bytes_copied += batch.total_size;
					}
					Err(e) => {
						// Clean up and return shutdown status
						if fs::try_exists(&target)
							.await
							.map_err(|e| JobError::IO(e.into()))?
						{
							fs::remove_file(&target)
								.await
								.map_err(|e| JobError::IO(e.into()))?;
						}

						return Ok(TaskStatus::Shutdown(Box::new(self.clone())));
					}
				}
			}
		}

		let duration = start.elapsed();
		let average_speed = if duration.as_secs() > 0 {
			bytes_copied / duration.as_secs()
		} else {
			bytes_copied
		};

		ctx.progress(vec![ProgressUpdate::Stats(serde_json::to_value(
			CopyProgress::Completed {
				files_copied,
				bytes_copied,
				total_duration: duration,
				average_speed,
			},
		)?)])
		.await;

		Ok(TaskStatus::Complete(CopyTaskStats {
			source: self.source.clone(),
			target: self.target.clone(),
			bytes_copied,
		}))
	}

	pub async fn serialize(&self) -> Result<Vec<u8>, rmp_serde::encode::Error> {
		rmp_serde::to_vec_named(self)
	}
}

#[derive(Debug)]
pub struct CopyTaskStats {
	pub source: SdPath,
	pub target: SdPath,
	pub bytes_copied: u64,
}

#[async_trait::async_trait]
impl Task for CopyTask {
	type Error = JobError;

	fn id(&self) -> TaskId {
		self.id
	}

	fn name(&self) -> &'static str {
		"copy"
	}

	fn weight(&self) -> u32 {
		// Directory operations are relatively lightweight
		1
	}

	async fn run(&mut self, ctx: &impl JobContext) -> Result<TaskStatus, JobError> {
		// Implementation remains the same...
		self.run(ctx).await
	}
}

#[async_trait::async_trait]
impl SerializableTask<JobError> for CopyTask {
	type SerializeError = rmp_serde::encode::Error;
	type DeserializeError = rmp_serde::decode::Error;
	type DeserializeCtx = (Arc<PrismaClient>, Arc<SyncManager>);

	async fn serialize(&self) -> Result<Vec<u8>, Self::SerializeError> {
		rmp_serde::to_vec_named(self)
	}

	async fn deserialize(
		bytes: &[u8],
		_ctx: &Self::DeserializeCtx,
	) -> Result<Self, Self::DeserializeError> {
		rmp_serde::from_slice(bytes)
	}
}
