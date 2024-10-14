mod move_to_trash;
mod remove;

use std::{
	future::Future,
	marker::PhantomData,
	path::{Path, PathBuf},
};

use itertools::Itertools;
use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_heavy_lifting::{
	job_system::{
		job::{Job, JobReturn, JobTaskDispatcher, ReturnStatus},
		SerializableJob, SerializedTasks,
	},
	Error, JobContext, JobName, NonCriticalError, OuterContext, ProgressUpdate,
};
use sd_core_prisma_helpers::file_path_with_object;
use sd_prisma::prisma::{file_path, location, PrismaClient};
use serde::{Deserialize, Serialize};

pub type MoveToTrashJob = DeleterJob<move_to_trash::MoveToTrashBehavior>;

pub type RemoveJob = DeleterJob<remove::RemoveBehavior>;

/// Specify how the [`Deleter`] should processed to delete a file
pub trait DeleteBehavior {
	fn delete(file: FileData) -> impl Future<Output = Result<(), ()>> + Send;

	fn delete_all<I>(files: I) -> impl Future<Output = Result<(), ()>> + Send
	where
		I: IntoIterator<Item = FileData> + Send,
		I::IntoIter: Send,
	{
		use futures_concurrency::future::Join;
		async {
			files
				.into_iter()
				.map(Self::delete)
				.collect::<Vec<_>>()
				.join()
				.await;
			Ok(())
		}
	}
}

// struct Erase; //  TODO(matheus-consoli):  ???

#[derive(Debug, Hash)]
pub struct DeleterJob<B: DeleteBehavior + std::hash::Hash> {
	location_id: location::id::Type,
	file_path_ids: Vec<file_path::id::Type>,
	steps: Option<()>,
	behavior: PhantomData<fn(B) -> B>, // variance: invariant, inherent Send + Sync
}

impl<B: DeleteBehavior + std::hash::Hash> DeleterJob<B> {
	pub const fn new(
		location_id: location::id::Type,
		file_path_ids: Vec<file_path::id::Type>,
	) -> Self {
		Self {
			location_id,
			file_path_ids,
			steps: None,
			behavior: PhantomData,
		}
	}
}

// ver como o indexer organizar suas tasks (se são criadas no run ou no new)

impl<B: DeleteBehavior + std::hash::Hash + 'static> Job for DeleterJob<B> {
	const NAME: JobName = JobName::Delete;
	// TODO(matheus-consoli): tracing
	async fn run<OuterCtx: OuterContext>(
		self,
		_: JobTaskDispatcher,
		ctx: impl JobContext<OuterCtx>,
	) -> Result<ReturnStatus, Error> {
		// TODO(matheus-consoli): bulk jobs as tasks
		let location_path = get_location_path_from_location_id(ctx.db(), self.location_id)
			.await
			.map_err(|_e| todo!("?, aka error handling"))
			.unwrap();

		// TODO(matheus-consoli): use a better query (get only the info we need)
		let files = get_many_files_datas(ctx.db(), location_path, &self.file_path_ids)
			.await
			.map_err(|_| todo!("FileSystemJobsError::from"))
			.unwrap();

		ctx.progress([ProgressUpdate::CompletedTaskCount(files.len() as _)])
			.await;

		let mut steps: Vec<Vec<_>> = {
			// TODO(matheus-consoli): remove this call to clone
			let temp = files.clone().into_iter();
			let ch = temp.chunks(50);

			ch.into_iter().map(|c| c.collect()).collect()
		};

		// if we have a single step, let's execute it, otherwise spawn background tasks
		let steps = if steps.len() == 1 {
			tracing::debug!("deleting fits in a single step, straight up executing it");
			B::delete_all(steps.pop().expect("we checked the lenght")).await;
			//  TODO(matheus-consoli):  error handling
			None
		} else {
			Some(())
		};

		// TODO(matheus-consoli): smart tasks, spawn tasks if there is too much files
		let mut r: Vec<()> = Vec::new();
		let mut n = 0;
		for file in files {
			// if let Err(e) = B::delete(file).await {
			// 	r.push(e);
			// }
			n += 1;
			ctx.progress([ProgressUpdate::TaskCount(n)]).await;
		}

		// TODO(matheus-consoli): inline this later
		let errors = r
			.into_iter()
			.map(|_| NonCriticalError::Deleter("TODO handle errors".into()))
			.collect();

		ctx.progress([ProgressUpdate::CompletedTaskCount(n)]).await;

		Ok(ReturnStatus::Completed(
			JobReturn::builder()
				.with_non_critical_errors(errors)
				.build(),
		))
	}
}

// TODO(matheus-consoli): add serialization once we add smart tasks
impl<OuterCtx: OuterContext, B: DeleteBehavior + std::hash::Hash + 'static>
	SerializableJob<OuterCtx> for DeleterJob<B>
{
	async fn serialize(self) -> Result<Option<Vec<u8>>, rmp_serde::encode::Error> {
		Ok(None)
	}

	async fn deserialize(
		serialized_job: &[u8],
		ctx: &OuterCtx,
	) -> Result<Option<(Self, Option<SerializedTasks>)>, rmp_serde::decode::Error> {
		Ok(None)
	}
}

type TODO = Box<dyn std::error::Error>;

/// Get the [`FileData`] related to every `file_path_id`
async fn get_many_files_datas(
	db: &PrismaClient,
	location_path: impl AsRef<Path>,
	file_path_ids: &[file_path::id::Type],
) -> Result<Vec<FileData>, TODO> {
	let location_path = location_path.as_ref();

	db._batch(
		file_path_ids
			.iter()
			.map(|file_path_id| {
				db.file_path()
					.find_unique(file_path::id::equals(*file_path_id))
					.include(file_path_with_object::include())
			})
			// FIXME:(fogodev -> Brendonovich) this collect is a workaround to a weird higher ranker lifetime error on
			// the _batch function, it should be removed once the error is fixed
			.collect::<Vec<_>>(),
	)
	.await?
	.into_iter()
	.zip(file_path_ids.iter())
	.map(|(maybe_file_path, file_path_id)| {
		maybe_file_path
			// TODO(matheus-consoli): proper error handling
			.ok_or_else(|| todo!())
			// .ok_or(FileSystemJobsError::FilePathIdNotFound(*file_path_id))
			.and_then(|path_data| {
				Ok(FileData {
					full_path: location_path.join(IsolatedFilePathData::try_from(&path_data)?),
					file_path: path_data,
				})
			})
	})
	.collect()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileData {
	pub file_path: file_path_with_object::Data,
	pub full_path: PathBuf,
}

pub async fn get_location_path_from_location_id(
	db: &PrismaClient,
	location_id: file_path::id::Type,
) -> Result<PathBuf, TODO> {
	db.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await
		.map_err(Into::into)
		.and_then(|maybe_location| {
			maybe_location
				// TODO(matheus-consoli): proper error handling
				.ok_or_else(|| todo!())
				// .ok_or(LocationError::IdNotFound(location_id))
				.and_then(|location| {
					location
						.path
						.map(PathBuf::from)
						// TODO(matheus-consoli): proper error handling
						.ok_or_else(|| todo!())
					// .ok_or(LocationError::MissingPath(location_id))
				})
		})
}

// TODO(matheus-consoli):
// - [ ] smart tasks
// - [ ] fix get_many_files_data to get only the necessary data
// - [ ] query for how many files there are inside a directory (for progress update)
