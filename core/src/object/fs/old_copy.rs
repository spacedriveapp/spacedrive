use crate::{
	invalidate_query,
	library::Library,
	old_job::{
		CurrentStep, JobError, JobInitOutput, JobReportUpdate, JobResult, JobRunMetadata,
		JobStepOutput, StatefulJob, WorkerContext,
	},
};

use sd_core_file_path_helper::{join_location_relative_path, IsolatedFilePathData};

use sd_prisma::prisma::{file_path, location};
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::{
	collections::HashSet,
	hash::Hash,
	path::{Path, PathBuf},
	sync::{Arc, Mutex},
	time::Duration,
};

use futures_concurrency::future::{Race, TryJoin};
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;
use tokio::fs;
use tracing::debug;

use super::{
	construct_target_filename, error::FileSystemJobsError, fetch_source_and_target_location_paths,
	find_available_filename_for_duplicate, get_file_data_from_isolated_file_path,
	get_many_files_datas, FileData,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OldFileCopierJobData {
	sources_location_path: PathBuf,
	total_size: u64,
	steps_len: usize,
}

#[derive(Serialize, Deserialize, Hash, Type, Debug)]
pub struct OldFileCopierJobInit {
	pub source_location_id: location::id::Type,
	pub target_location_id: location::id::Type,
	pub sources_file_path_ids: Vec<file_path::id::Type>,
	pub target_location_relative_directory_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
enum CopierStepKind {
	CreateDirs(CreateDirs),
	CopyFiles(CopyFiles),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct CopyFiles;

impl CopyFiles {
	async fn copy_files(
		files: &[Copy],
		jobmeta: Arc<Mutex<OldFileCopierJobMetadata>>,
	) -> Result<(), JobError> {
		// NOTE(matheus-consoli): if a step contains multiple files with the same name,
		// for example `[file (1), file (2), file (3)]`, they all may be get renamed to the same
		// new file (`file (4)`) as the decision for the new file name and the creation of such file
		// all happens concurrently.
		// this HashSet introduces a point of synchronization so we guarantee that all files have
		// unique names - this is not the ideal solution, but it's a quick one.
		let renamed_files_in_this_step = Arc::new(Mutex::new(HashSet::new()));

		files
			.iter()
			.map(
				|Copy {
				     source,
				     source_size,
				     target_full_path,
				 }| {
					let jobmeta = Arc::clone(&jobmeta);
					let renamed_files_in_this_step = Arc::clone(&renamed_files_in_this_step);
					async move {
						let target = loop {
							let target =
								OldFileCopierJobStep::find_available_name(&target_full_path)
									.await?;

							let mut cache = renamed_files_in_this_step
								.lock()
								.expect("failed to get lock for internal cache");
							if cache.contains(&target) {
								// file name is taken, try again
								continue;
							} else {
								cache.insert(target.clone());
								break target;
							}
						};
						fs::copy(&source.full_path, &target).await.map_err(|e| {
							let source = source.full_path.clone();
							FileIOError::from((source, e))
						})?;

						let mut meta = jobmeta
							.lock()
							.expect("failed to get the lock for the list of files to copy");
						let accumulated_copied_size = meta.accumulated_copied_size + source_size;
						let copied_files_count = meta.copied_files_count + 1;
						meta.update(OldFileCopierJobMetadata {
							accumulated_copied_size,
							copied_files_count,
						});

						Ok::<_, JobError>(())
					}
				},
			)
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		Ok(())
	}
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct CreateDirs;

impl CreateDirs {
	/// Create the directories
	async fn create_dir_structure(files: &[Copy]) -> Result<(), JobError> {
		// TODO(matheus-consoli): when the directory name conflicts, what should we do?
		// same as find_available...?
		files
			.iter()
			.map(|file| async move {
				fs::create_dir_all(&file.target_full_path)
					.await
					.map_err(|e| FileIOError::from((file.target_full_path.clone(), e)))
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;
		Ok(())
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Copy {
	source: FileData,
	source_size: u64,
	target_full_path: Box<Path>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OldFileCopierJobStep {
	files: Box<[Copy]>,
	target_location_path: Box<Path>,
	copy_kind: CopierStepKind,
}

impl OldFileCopierJobStep {
	async fn find_available_name(path: impl AsRef<Path>) -> Result<PathBuf, JobError> {
		let path = path.as_ref();
		match fs::try_exists(&path).await {
			Ok(true) => {
				// file already exists, try finding a better name
				find_available_filename_for_duplicate(&path)
					.await
					.map_err(Into::into)
			}
			Ok(false) => {
				// nothing todo, file name is available
				Ok(path.to_owned())
			}
			Err(e) => Err(FileIOError::from((path, e)).into()),
		}
	}
}

fn progress(ctx: &WorkerContext, msgs: impl IntoIterator<Item = CopierUpdate>) {
	let updates = msgs.into_iter().map(Into::into).collect();

	ctx.progress(updates);
}

#[derive(Debug, Clone)]
enum CopierUpdate {
	Start,
	TotalSize(String),
	FileCount(usize),
	TotalProgress(u64),
	ProgressPerFile(String),
	FinishedWithPercetage(u64),
}

impl From<CopierUpdate> for JobReportUpdate {
	fn from(value: CopierUpdate) -> Self {
		match value {
			CopierUpdate::Start => {
				const HUNDRED_PERCENT: usize = 100;
				JobReportUpdate::TaskCount(HUNDRED_PERCENT)
			}
			CopierUpdate::FinishedWithPercetage(task_progress) => JobReportUpdate::TaskCount(
				task_progress
					.try_into()
					.expect("should be able to convert a `u64` to `usize`"),
			),
			CopierUpdate::TotalSize(size) => JobReportUpdate::Message(size.to_owned()),
			CopierUpdate::FileCount(count) => JobReportUpdate::Info(count.to_string()),
			CopierUpdate::ProgressPerFile(per_file) => JobReportUpdate::Phase(per_file.to_owned()),
			CopierUpdate::TotalProgress(progressed_tasks) => JobReportUpdate::CompletedTaskCount(
				progressed_tasks
					.try_into()
					.expect("should be able to convert a `u64` to `usize`"),
			),
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct OldFileCopierJobMetadata {
	accumulated_copied_size: u64,
	copied_files_count: u64,
}

impl JobRunMetadata for OldFileCopierJobMetadata {
	fn update(&mut self, metadata: Self) {
		*self = metadata;
	}
}

#[async_trait::async_trait]
impl StatefulJob for OldFileCopierJobInit {
	type Data = OldFileCopierJobData;
	type Step = OldFileCopierJobStep;
	type RunMetadata = OldFileCopierJobMetadata;

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

		let (sources_location_path, targets_location_path) =
			fetch_source_and_target_location_paths(
				db,
				init.source_location_id,
				init.target_location_id,
			)
			.await?;

		let files =
			get_many_files_datas(db, &sources_location_path, &init.sources_file_path_ids).await?;

		if let Some(missing_field) = files
			.iter()
			.find_map(|file| maybe_missing(file.file_path.is_dir, "file.is_dir").err())
		{
			return Err(missing_field.into());
		}

		let archives = files
			.into_iter()
			.map(|file_data| async {
				// add the currently viewed subdirectory to the location root
				let mut full_target_path = join_location_relative_path(
					&targets_location_path,
					&init.target_location_relative_directory_path,
				);

				full_target_path.push(construct_target_filename(&file_data)?);

				Ok::<_, FileSystemJobsError>((file_data, full_target_path))
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		let (mut dirs, mut files): (Vec<_>, _) = archives.into_iter().partition(|file| {
			file.0
				.file_path
				.is_dir
				.expect("we tested that all file paths have the `is_dir` field")
		});

		let mut steps = Vec::with_capacity(dirs.len() + files.len());

		// first step: create all directories using a single job
		if !dirs.is_empty() {
			let (more_dirs, more_files) = unfold_diretory(&dirs).await?;

			let more_dirs = more_dirs
				.into_iter()
				.map(|dir| async {
					let iso = IsolatedFilePathData::new(
						init.source_location_id,
						&sources_location_path,
						dir.source,
						true, // is dir
					)
					.map_err(FileSystemJobsError::from)?;
					let file_data = get_file_data_from_isolated_file_path(
						&ctx.library.db,
						&sources_location_path,
						&iso,
					)
					.await?;
					Ok::<_, JobError>((file_data, dir.dest))
				})
				.collect::<Vec<_>>()
				.try_join()
				.await?;
			dirs.extend(more_dirs);

			let (dir_source_file_data, dir_target_full_path) =
				dirs.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();

			let step_files = dir_source_file_data
				.into_iter()
				.zip(dir_target_full_path.into_iter())
				.map(|(source, target_full_path)| Copy {
					source,
					source_size: 0,
					target_full_path: target_full_path.into_boxed_path(),
				})
				.collect();

			let create_dirs_step = OldFileCopierJobStep {
				files: step_files,
				target_location_path: targets_location_path.clone().into_boxed_path(),
				copy_kind: CopierStepKind::CreateDirs(CreateDirs),
			};
			steps.push(create_dirs_step);

			let more_files = more_files
				.into_iter()
				.map(|file| async {
					let iso = IsolatedFilePathData::new(
						init.source_location_id,
						&sources_location_path,
						file.source,
						false, // is dir
					)
					.map_err(FileSystemJobsError::from)?;
					let file_data = get_file_data_from_isolated_file_path(
						&ctx.library.db,
						&sources_location_path,
						&iso,
					)
					.await?;
					Ok::<_, JobError>((file_data, file.dest))
				})
				.collect::<Vec<_>>()
				.try_join()
				.await;

			if let Err(e) = more_files.as_ref() {
				// the file is not indexed
				tracing::error!(?e, "job init failed");
			}

			files.extend(more_files?);
		};

		// remaining steps: delegate to the copy strategist to decide how to organize
		// the steps that copies the files
		let steps_to_create_files = file_copy_strategist(files, &targets_location_path).await?;
		steps.extend(steps_to_create_files);

		let total_size = steps
			.iter()
			.filter(|step| matches!(step.copy_kind, CopierStepKind::CopyFiles(_)))
			.map(|step| step.files.iter().map(|file| file.source_size).sum::<u64>())
			.sum::<u64>();

		let file_count = steps
			.iter()
			.filter(|step| matches!(step.copy_kind, CopierStepKind::CopyFiles(_)))
			.map(|step| step.files.len())
			.sum::<usize>();

		let updates = [
			CopierUpdate::Start,
			CopierUpdate::FileCount(file_count),
			CopierUpdate::TotalSize(total_size.to_string()),
		];
		progress(ctx, updates);

		*data = Some(OldFileCopierJobData {
			sources_location_path,
			total_size,
			steps_len: steps.len(),
		});

		Ok(steps.into())
	}

	#[tracing::instrument(
		skip_all,
		fields(
			step.kind = ?step.step.copy_kind,
			step.n = step.step_number,
			progress = jobmeta.accumulated_copied_size
		)
	)]
	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		step: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		jobmeta: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let files = &step.step.files;
		let acc_copied_size = jobmeta.accumulated_copied_size;
		let total_size = data.total_size;
		let jobmeta = Arc::new(Mutex::new(jobmeta.clone()));

		let transfer = {
			let jobmeta = Arc::clone(&jobmeta);
			async move {
				match step.step.copy_kind {
					CopierStepKind::CreateDirs(CreateDirs) => {
						CreateDirs::create_dir_structure(&step.step.files).await?;
					}
					CopierStepKind::CopyFiles(CopyFiles) => {
						CopyFiles::copy_files(&step.step.files, jobmeta).await?;
					}
				};
				Ok::<_, JobError>(())
			}
		};

		let report = async move {
			let mut finished = vec![false; files.len()];
			let mut step_copied = vec![0; files.len()];
			let relative_paths: Vec<&Path> = files
				.iter()
				.map(|f| {
					f.target_full_path
						.strip_prefix(&step.step.target_location_path)
						.unwrap_or(&f.target_full_path)
				})
				.collect();

			loop {
				for (((file, relative_path), copied), is_file_done) in files
					.iter()
					.zip(relative_paths.iter())
					.zip(step_copied.iter_mut())
					.zip(finished.iter_mut())
					.filter(|(_, is_file_done)| !**is_file_done)
				{
					let Ok(transfering) = fs::metadata(&file.target_full_path).await else {
						// file may not have been created yet
						continue;
					};

					let file_percentage =
						(transfering.len() as f64 / file.source_size as f64) * 100.0;
					let file_percentage = file_percentage.round();

					let msg = format!("{file_percentage}% of {:?}", relative_path);
					progress(ctx, [CopierUpdate::ProgressPerFile(msg)]);

					*copied = transfering.len();
					if transfering.len() == file.source_size {
						*is_file_done = true;
					}
				}

				let copied_in_step = step_copied.iter().sum::<u64>();
				let total_percentage =
					((copied_in_step + acc_copied_size) as f64 / total_size as f64) * 100.;
				let per = total_percentage.round() as u64;
				progress(ctx, [CopierUpdate::TotalProgress(per)]);

				// wait for progress
				tokio::time::sleep(Duration::from_millis(200)).await;
			}
		};

		(transfer, report).race().await?;

		if data.steps_len == step.step_number + 1 {
			let jobmeta = jobmeta
				.lock()
				.expect("failed to get the lock for job metadata");
			progress(
				ctx,
				[CopierUpdate::FinishedWithPercetage(
					jobmeta.copied_files_count,
				)],
			);
		}

		let jobmeta = Arc::into_inner(jobmeta)
			.expect("all the other copies should have been dropped by this point")
			.into_inner()
			.expect("the Mutex shouldn't be poisoned");

		// we've calculated all steps up ahead
		Ok(jobmeta.into())
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

/// Gather information about the list of files and decide what is the best
/// approach to organize them into steps.
async fn file_copy_strategist(
	files: Vec<(FileData, PathBuf)>,
	location_path: &Path,
) -> Result<Vec<OldFileCopierJobStep>, JobError> {
	// maximum size in bytes per step (800)
	const MAX_TOTAL_SIZE_PER_STEP: u64 = 1024 * 1024 * 800;
	// max quantity of files per step
	const MAX_FILES_PER_STEP: usize = 20;

	debug!("generating steps to copy files");

	let mut metadata = files
		.into_iter()
		.map(|(data, path)| async move {
			let meta = tokio::fs::metadata(&data.full_path)
				.await
				.map_err(|e| FileIOError::from((data.full_path.clone(), e)))?;
			Ok::<_, JobError>((meta.len(), data, path))
		})
		.collect::<Vec<_>>()
		.try_join()
		.await?;

	// sort by size
	metadata.sort_unstable_by_key(|m| m.0);

	let mut metadata = metadata.into_iter().peekable();
	let mut steps = Vec::new();

	loop {
		let mut sum = 0;
		let mut files = Vec::with_capacity(MAX_FILES_PER_STEP);

		while let Some((source_size, source, path)) = metadata.next_if(|(len, _, _)| {
			files.len() < MAX_FILES_PER_STEP && len + sum <= MAX_TOTAL_SIZE_PER_STEP || sum == 0
		}) {
			sum += source_size;
			files.push(Copy {
				source,
				source_size,
				target_full_path: path.into_boxed_path(),
			});
		}

		steps.push(OldFileCopierJobStep {
			files: files.into_boxed_slice(),
			target_location_path: location_path.into(),
			copy_kind: CopierStepKind::CopyFiles(CopyFiles),
		});

		if metadata.peek().is_none() {
			// nothing left to do, all files are grouped into steps
			break;
		}
	}

	Ok(steps)
}

async fn unfold_diretory(
	dirs: &[(FileData, PathBuf)],
) -> Result<(Vec<NewEntry>, Vec<NewEntry>), JobError> {
	let mut unfolded_dirs = Vec::new();
	let mut unfolded_files = Vec::new();

	let mut dirs = Vec::from_iter(
		dirs.iter()
			.map(|(file_data, path)| (file_data.full_path.clone(), path.clone())),
	);

	loop {
		if dirs.is_empty() {
			break;
		}
		let unfolds = dirs
			.iter()
			.map(|(file_data, target_full_path)| async move {
				let target_full_path = target_full_path.clone();

				let mut to_look = Vec::new();
				let mut more_dirs = Vec::new();
				let mut more_files = Vec::new();
				let mut read_dir = fs::read_dir(file_data)
					.await
					.map_err(|e| FileIOError::from((file_data.clone(), e)))?;

				while let Some(children_entry) = read_dir
					.next_entry()
					.await
					.map_err(|e| FileIOError::from((file_data.clone(), e)))?
				{
					let children_path = &children_entry.path();
					let relative_path = children_path.strip_prefix(file_data)
						.expect("We got the children path from the `read_dir`, so it should be a child of the source path");
					let target_children_full_path = target_full_path.join(relative_path);
					let metadata = fs::metadata(children_path)
						.await
						.map_err(|e| FileIOError::from((file_data.clone(), e)))?;
					if metadata.is_dir() {
						to_look.push((children_path.clone(), target_children_full_path.clone()));
						let dir = NewEntry {
							source: children_path.clone(),
							dest: target_children_full_path,
						};
						more_dirs.push(dir);
					} else {
						let file = NewEntry {
							source: children_path.clone(),
							dest: target_children_full_path,
						};
						more_files.push(file);
					}
				}

				Ok::<_, JobError>((to_look, more_dirs, more_files))
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		dirs.clear();
		unfolds
			.into_iter()
			.for_each(|(keep_looking, more_dirs, more_files)| {
				dirs.extend(keep_looking);
				unfolded_dirs.extend(more_dirs);
				unfolded_files.extend(more_files);
			});
	}

	Ok((unfolded_dirs, unfolded_files))
}

#[derive(Debug)]
struct NewEntry {
	source: PathBuf,
	dest: PathBuf,
}
