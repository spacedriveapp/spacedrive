use crate::{
	invalidate_query,
	library::Library,
	old_job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunErrors, JobStepOutput, StatefulJob,
		WorkerContext,
	},
};

use itertools::Itertools;
use sd_core_file_path_helper::{join_location_relative_path, IsolatedFilePathData};

use sd_prisma::prisma::{file_path, location};
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::{fmt, hash::Hash, path::PathBuf};

use futures_concurrency::future::TryJoin;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use tokio::{fs, io};
use tracing::{debug, trace, warn};

use super::{
	construct_target_filename, error::FileSystemJobsError, fetch_source_and_target_location_paths,
	find_available_filename_for_duplicate, get_file_data_from_isolated_file_path,
	get_many_files_datas, FileData,
};

// spawn one job
// this job will receive the data from a channel
// the data will be a Batch of files
// process the batch
// race it with the interrupter
//
// open questions:
// - when to spawn the "actor"?

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OldFileCopierJobData {
	sources_location_path: PathBuf,
}

#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct OldFileCopierJobInit {
	pub source_location_id: location::id::Type,
	pub target_location_id: location::id::Type,
	pub sources_file_path_ids: Vec<file_path::id::Type>,
	pub target_location_relative_directory_path: PathBuf,
}

enum Msg {
	Start(Vec<PathBuf>, async_channel::Sender<&'static str>),
	Finish,
}

impl OldFileCopierJobInit {
	// TODO(matheus-consoli): return two channels, a sender and a watcher (to communicate the status)
	async fn spawn_background_task() -> Result<async_channel::Sender<Msg>, ()> {
		let (rx, tx) = async_channel::unbounded();

		// spawn the info-gathernig service
		// recieves a batch of files and keeps reporting its size

		// TODO(matheus-consoli): use our own jobs instead of tokio's
		tokio::task::spawn(async move {
			loop {
				match tx.recv().await {
					Ok(Msg::Finish) => break,
					Ok(Msg::Start(files, report_to)) => {
						debug!(?files, "watching these files");
						_ = report_to.send("hellow").await;
					}
					Err(_) => {
						// TODO(matheus-consoli): idk what does it means
					}
				}
			}
		});

		Ok(rx)
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OldFileCopierJobStep {
	pub source_file_data: Vec<FileData>,
	pub target_full_path: Vec<PathBuf>, // deal with the errors now :D
	// todo: more info about the transfer
	#[serde(skip)]
	respond_to: Option<async_channel::Sender<Msg>>,
}

#[async_trait::async_trait]
impl StatefulJob for OldFileCopierJobInit {
	type Data = OldFileCopierJobData;
	type Step = OldFileCopierJobStep;
	type RunMetadata = ();

	const NAME: &'static str = "file_copier";
	const IS_BATCHED: bool = true;

	fn target_location(&self) -> location::id::Type {
		self.target_location_id
	}

	async fn init(
		&self,
		ctx: &WorkerContext,
		data: &mut Option<Self::Data>,
	) -> Result<JobInitOutput<Self::RunMetadata, Self::Step>, JobError> {
		let init = self;
		let Library { db, .. } = &*ctx.library;

		let rx = Self::spawn_background_task().await.unwrap();

		let (sources_location_path, targets_location_path) =
			fetch_source_and_target_location_paths(
				db,
				init.source_location_id,
				init.target_location_id,
			)
			.await?;

		// maybe it's here that we are f* things up
		let steps = get_many_files_datas(db, &sources_location_path, &init.sources_file_path_ids)
			.await?
			.into_iter()
			.map(|source_file_data| async {
				// add the currently viewed subdirectory to the location root
				let mut target_full_path = join_location_relative_path(
					&targets_location_path,
					&init.target_location_relative_directory_path,
				);

				target_full_path.push(construct_target_filename(&source_file_data)?);

				if source_file_data.full_path == target_full_path {
					target_full_path =
						find_available_filename_for_duplicate(target_full_path).await?;
				}

				Ok::<_, FileSystemJobsError>(OldFileCopierJobStep {
					source_file_data,
					target_full_path,
					respond_to: Some(rx.clone()),
				})
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		*data = Some(OldFileCopierJobData {
			sources_location_path,
		});

		Ok(steps.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		CurrentStep {
			step:
				OldFileCopierJobStep {
					source_file_data,
					target_full_path,
					respond_to,
				},
			..
		}: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;

		debug!("executing_step");

		// this is called at every file

		if maybe_missing(source_file_data.file_path.is_dir, "file_path.is_dir")? {
			let mut more_steps = Vec::new();

			fs::create_dir_all(target_full_path)
				.await
				.map_err(|e| FileIOError::from((target_full_path, e)))?;

			let mut read_dir = fs::read_dir(&source_file_data.full_path)
				.await
				.map_err(|e| FileIOError::from((&source_file_data.full_path, e)))?;

			while let Some(children_entry) = read_dir
				.next_entry()
				.await
				.map_err(|e| FileIOError::from((&source_file_data.full_path, e)))?
			{
				let children_path = children_entry.path();
				let target_children_full_path = target_full_path.join(
                    children_path
                        .strip_prefix(&source_file_data.full_path)
                        .expect("We got the children path from the read_dir, so it should be a child of the source path"),
                );

				match get_file_data_from_isolated_file_path(
					&ctx.library.db,
					&data.sources_location_path,
					&IsolatedFilePathData::new(
						init.source_location_id,
						&data.sources_location_path,
						&children_path,
						children_entry
							.metadata()
							.await
							.map_err(|e| FileIOError::from((&children_path, e)))?
							.is_dir(),
					)
					.map_err(FileSystemJobsError::from)?,
				)
				.await
				{
					Ok(source_file_data) => {
						// Currently not supporting file_name suffixes children files in a directory being copied
						more_steps.push(OldFileCopierJobStep {
							target_full_path: target_children_full_path,
							source_file_data,
							respond_to: respond_to.clone(),
						});
					}
					Err(FileSystemJobsError::FilePathNotFound(path)) => {
						// FilePath doesn't exist in the database, it possibly wasn't indexed, so we skip it
						warn!(
							path = %path.display(),
							"Skipping duplicating as it wasn't indexed;",
						);
					}
					Err(e) => return Err(e.into()),
				}
			}

			Ok(more_steps.into())
		} else {
			match fs::metadata(target_full_path).await {
				Ok(_) => {
					// Already exist a file with this name, so we need to find an available name
					match find_available_filename_for_duplicate(target_full_path).await {
						Ok(new_path) => {
							fs::copy(&source_file_data.full_path, &new_path)
								.await
								// Using the ? here because we don't want to increase the completed task
								// count in case of file system errors
								.map_err(|e| FileIOError::from((new_path, e)))?;

							Ok(().into())
						}

						Err(FileSystemJobsError::FailedToFindAvailableName(path)) => {
							Ok(JobRunErrors(vec![
								FileSystemJobsError::WouldOverwrite(path).to_string()
							])
							.into())
						}

						Err(e) => Err(e.into()),
					}
				}
				Err(e) if e.kind() == io::ErrorKind::NotFound => {
					trace!(
						source = %source_file_data.full_path.display(),
						target = %target_full_path.display(),
						"Copying source -> target;",
					);

					fs::copy(&source_file_data.full_path, &target_full_path)
						.await
						// Using the ? here because we don't want to increase the completed task
						// count in case of file system errors
						.map_err(|e| FileIOError::from((target_full_path, e)))?;

					Ok(().into())
				}
				Err(e) => Err(FileIOError::from((target_full_path, e)).into()),
			}
		}
	}

	async fn finalize(
		&self,
		ctx: &WorkerContext,
		_data: &Option<Self::Data>,
		_run_metadata: &Self::RunMetadata,
	) -> JobResult {
		let init = self;

		invalidate_query!(ctx.library, "search.paths");

		Ok(Some(json!({ "init": init })))
	}
}
