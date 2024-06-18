use crate::{
	invalidate_query,
	library::Library,
	old_job::{
		CurrentStep, JobError, JobInitOutput, JobResult, JobRunErrors, JobStepOutput, StatefulJob,
		WorkerContext,
	},
};

use futures::stream::FuturesUnordered;
use itertools::Itertools;
use sd_core_file_path_helper::{join_location_relative_path, IsolatedFilePathData};

use sd_prisma::prisma::{file_path, location, PrismaClient};
use sd_utils::{db::maybe_missing, error::FileIOError};

use std::{
	fmt,
	hash::Hash,
	path::{Path, PathBuf},
	sync::{Arc, Mutex},
};

use futures_concurrency::future::{Join, TryJoin};
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

const CHUNKS_SIZE: usize = 2;

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

#[derive(Serialize, Deserialize, Debug, Default)]
enum CopyBehavior {
	#[default]
	FsCopy,
	ParallelByChunks,
}

#[derive(Serialize, Deserialize, Debug)]
enum CopierStepKind {
	CreateDirs,
	CopyFiles,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OldFileCopierJobStep {
	pub source_file_data: Vec<FileData>,
	pub target_full_path: Vec<PathBuf>,
	pub copy_step_kind: CopierStepKind,
	pub copy_behavior: CopyBehavior,
	// #[serde(skip)]
	// respond_to: Option<async_channel::Sender<Msg>>,
}

impl OldFileCopierJobStep {
	async fn copy_files(&self) {
		// let source_file_data = &self.source_file_data[0];
		// let target_full_path = &self.target_full_path[0];

		// let mut idk = vec![];

		// let r = match fs::metadata(target_full_path).await {
		// 	Ok(_) => {
		// 		// Already exist a file with this name, so we need to find an available name
		// 		match find_available_filename_for_duplicate(target_full_path).await {
		// 			Ok(new_path) => {
		// 				fs::copy(&source_file_data.full_path, &new_path)
		// 					.await
		// 					// Using the ? here because we don't want to increase the completed task
		// 					// count in case of file system errors
		// 					.map_err(|e| FileIOError::from((new_path, e)))?;

		// 				Ok(().into())
		// 			}

		// 			Err(FileSystemJobsError::FailedToFindAvailableName(path)) => Ok(JobRunErrors(
		// 				vec![FileSystemJobsError::WouldOverwrite(path).to_string()],
		// 			)
		// 			.into()),

		// 			Err(e) => Err(e.into()),
		// 		}
		// 	}
		// 	Err(e) if e.kind() == io::ErrorKind::NotFound => {
		// 		trace!(
		// 			"Copying from {} to {}",
		// 			source_file_data.full_path.display(),
		// 			target_full_path.display()
		// 		);

		// 		fs::copy(&source_file_data.full_path, &target_full_path)
		// 			.await
		// 			// Using the ? here because we don't want to increase the completed task
		// 			// count in case of file system errors
		// 			.map_err(|e| FileIOError::from((target_full_path, e)))?;

		// 		Ok(().into())
		// 	}
		// 	Err(e) => Err(FileIOError::from((target_full_path, e)).into()),
		// };
		// idk.push(r);
	}

	/// Create the directory and return the new steps necessary to copy its contents
	async fn prepare_directory(
		&self,
		source_location_id: location::id::Type,
		sources_location_path: impl AsRef<Path>,
		db: Arc<PrismaClient>,
	) -> Vec<()> {
		// options:
		// - self.source_file_data.filter(is_path)
		// - self.source_file_data for each

		// let mut more_steps = vec![];

		// fs::create_dir_all(target_full_path)
		// 	.await
		// 	.map_err(|e| FileIOError::from((target_full_path, e)))
		// 	.unwrap();

		// let mut read_dir = fs::read_dir(&source_file_data.full_path)
		// 	.await
		// 	.map_err(|e| FileIOError::from((&source_file_data.full_path, e)))
		// 	.unwrap();

		// while let Some(children_entry) = read_dir
		// 	.next_entry()
		// 	.await
		// 	.map_err(|e| FileIOError::from((&source_file_data.full_path, e)))
		// 	.unwrap()
		// {
		// 	let children_path = children_entry.path();
		// 	let target_children_full_path = target_full_path.join(
		//             children_path
		//                 .strip_prefix(&source_file_data.full_path)
		//                 .expect("We got the children path from the read_dir, so it should be a child of the source path"),
		//         );

		// 	let iso_file_path = IsolatedFilePathData::new(
		// 		source_location_id,
		// 		&sources_location_path,
		// 		&children_path,
		// 		children_entry
		// 			.metadata()
		// 			.await
		// 			.map_err(|e| FileIOError::from((&children_path, e)))
		// 			.unwrap()
		// 			.is_dir(),
		// 	)
		// 	.map_err(FileSystemJobsError::from)
		// 	.unwrap();
		// 	match get_file_data_from_isolated_file_path(&db, &sources_location_path, &iso_file_path)
		// 		.await
		// 	{
		// 		Ok(source_file_data) => {
		// 			// Currently not supporting file_name suffixes children files in a directory being copied
		// 			more_steps.push(OldFileCopierJobStep {
		// 				target_full_path: vec![target_children_full_path],
		// 				source_file_data: vec![source_file_data],
		// 				respond_to: self.respond_to.clone(),
		// 			});
		// 		}
		// 		Err(FileSystemJobsError::FilePathNotFound(path)) => {
		// 			// FilePath doesn't exist in the database, it possibly wasn't indexed, so we skip it
		// 			warn!(
		// 				"Skipping duplicating {} as it wasn't indexed",
		// 				path.display()
		// 			);
		// 		}
		// 		Err(e) => todo!(" return Err(e.into())"),
		// 	}
		// }

		vec![]
	}
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
		debug!("<OldFileCopierJobInit as StatefulJob>::init");
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

				if file_data.full_path == full_target_path {
					full_target_path =
						find_available_filename_for_duplicate(full_target_path).await?;
				}

				Ok::<_, FileSystemJobsError>((file_data, full_target_path))
			})
			.collect::<Vec<_>>()
			.try_join()
			.await?;

		let (mut dirs, mut files) = archives.into_iter().partition::<Vec<_>, _>(|file| {
			file.0
				.file_path
				.is_dir
				.expect("we tested that all file paths have the `is_dir` field")
		});

		let mut steps = Vec::with_capacity(dirs.len() + files.len());

		// first step: create all directories using a single job
		if !dirs.is_empty() {
			let (more_dirs, more_files) = unfold_diretory(&dirs).await.unwrap();

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
				.await
				.unwrap();
			dirs.extend(more_dirs);
			let (dir_source_file_data, dir_target_full_path) = dirs.into_iter().unzip();

			let create_dirs_step = OldFileCopierJobStep {
				source_file_data: dir_source_file_data,
				target_full_path: dir_target_full_path,
				copy_step_kind: CopierStepKind::CopyFiles,
				copy_behavior: CopyBehavior::FsCopy,
				// respond_to: Some(rx.clone()),
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
				.await
				.unwrap();
			files.extend(more_files);
		};

		let steps_to_create_files = file_copy_strategist(files).await.unwrap();
		steps.extend(steps_to_create_files);

		*data = Some(OldFileCopierJobData {
			sources_location_path,
		});

		Ok(steps.into())
	}

	async fn execute_step(
		&self,
		ctx: &WorkerContext,
		step: CurrentStep<'_, Self::Step>,
		data: &Self::Data,
		_: &Self::RunMetadata,
	) -> Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError> {
		let init = self;

		// TODO(matheus-consoli): remove this
		let source_file_data: &[FileData] = step.step.source_file_data.as_ref();
		let target_full_path: &[PathBuf] = step.step.target_full_path.as_ref();

		debug!(
			bulk_len = source_file_data.len(),
			"||||||||| executing_step"
		);

		// TODO(matheus-consoli): questions
		// 1) how the files are structured
		// 2) how many files can i get at once
		//   - this will help me to understand how to parallelize the job

		// TODO(matheus-consoli): support multiple mechanisms to copy files
		//  - copying many small files
		//  - copying gigantic files

		// continue bulking here
		// init is already bulking

		// the following steps are thinking about a single file
		// let's make it do multiple files

		// TODO(matheus-consoli): prob not needed anymore
		let mut idk: Vec<Result<JobStepOutput<Self::Step, Self::RunMetadata>, JobError>> = vec![];
		// TODO(matheus-consoli): use futures-concurrency `.co` feature instead
		for (source_file_data, target_full_path) in source_file_data
			.into_iter()
			.zip(target_full_path.into_iter())
		{
			if maybe_missing(source_file_data.file_path.is_dir, "file_path.is_dir").unwrap() {
				// handle the next steps
				_ = step
					.step
					.prepare_directory(
						init.source_location_id,
						&data.sources_location_path,
						Arc::clone(&ctx.library.db),
					)
					.await;
			} else {
				step.step.copy_files().await;
			}
		}

		todo!()
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
/// approach to organize the steps to copy them.
///
/// # Rules
///
/// - lots of small files: create steps containing groups of 20 files, or up to 200M.
///   a file is considered small if it have a size bellow 10M.
/// - medium files: idk yet
/// - very large files: a file is considered large if it passes the mark of 2G.
///   each large file is its own step, and is copied in a parallel manner.
async fn file_copy_strategist(
	files: Vec<(FileData, PathBuf)>,
) -> Result<Vec<OldFileCopierJobStep>, ()> {
	let mut metadata = files
		.iter()
		.map(|(data, path)| async move {
			let meta = tokio::fs::metadata(path).await?;
			Ok::<_, std::io::Error>((meta.len(), data, path))
		})
		.collect::<Vec<_>>()
		.try_join()
		.await
		.unwrap();

	//
	// TODO(matheus-consoli): maybe also receive the directories, and read the tree of files ahead of time
	// instead of reading on each step

	// let total_files = files.len();
	// let total_size = metadata
	// 	.iter()
	// 	.fold(0u64, |acc, metadata| acc.saturating_add(metadata.0));

	metadata.sort_unstable_by_key(|m| m.0);

	let result = files // use metadata instead
		.chunks(10)
		.into_iter()
		.map(|metadata| {
			let (source_file_data, target_full_path) = metadata.into_iter().cloned().unzip();
			OldFileCopierJobStep {
				source_file_data,
				target_full_path,
				copy_step_kind: CopierStepKind::CopyFiles,
				copy_behavior: CopyBehavior::FsCopy,
				// respond_to: None,
			}
		})
		.into_iter()
		.collect::<Vec<_>>();
	Ok(result)

	// each step can use a different approach to copy, so we can organize different steps
	// and can assume that its base structure (directories) are already created
	//
	// ideas:
	// - group multiple small files in a single job (up to some size, tbd)
	// - group big files up to a certain size in a single job
	// - make very big files utilize a copy pattern of multiple threads copying different sections in parallel
}

/// TODO(matheus-consoli): DOCUMENTATION
async fn unfold_diretory(
	dirs: &[(FileData, PathBuf)],
) -> Result<(Vec<NewEntry>, Vec<NewEntry>), ()> {
	let more_dirs = Arc::new(Mutex::new(Vec::new()));
	let more_files = Arc::new(Mutex::new(Vec::new()));

	let mut dirs = Vec::from_iter(
		dirs.iter()
			.map(|(file_data, path)| (file_data.full_path.clone(), path.clone())),
	);

	loop {
		if dirs.is_empty() {
			break;
		}
		let keep_looking = dirs
			.iter()
			.map(|(file_data, target_full_path)| async {
				let file_data = file_data.clone();
				let target_full_path = target_full_path.clone();

				let mut to_look = Vec::new();
				let mut read_dir = fs::read_dir(&file_data).await.unwrap();

				while let Some(children_entry) = read_dir.next_entry().await.unwrap() {
					let children_path = &children_entry.path();
					let relative_path = children_path.strip_prefix(&file_data).unwrap();
					//.expect("We got the children path from the `read_dir`, so it should be a child of the source path");
					let target_children_full_path = target_full_path.join(relative_path);
					let metadata = fs::metadata(children_path).await.unwrap();
					if metadata.is_dir() {
						to_look.push((children_path.clone(), target_children_full_path.clone()));
						let dir = NewEntry {
							source: children_path.clone(),
							dest: target_children_full_path,
						};
						more_dirs.lock().unwrap().push(dir);
					} else {
						let file = NewEntry {
							source: children_path.clone(),
							dest: target_children_full_path,
						};
						more_files.lock().unwrap().push(file);
					}
				}

				Ok::<_, JobError>(to_look)
			})
			.collect::<Vec<_>>()
			.try_join()
			.await
			.unwrap();

		dirs.clear();
		dirs.extend(keep_looking.into_iter().flatten());
	}

	let more_dirs = Arc::try_unwrap(more_dirs).unwrap().into_inner().unwrap();
	let more_files = Arc::try_unwrap(more_files).unwrap().into_inner().unwrap();
	Ok((more_dirs, more_files))
}

#[derive(Debug)]
struct NewEntry {
	source: PathBuf,
	dest: PathBuf,
}
