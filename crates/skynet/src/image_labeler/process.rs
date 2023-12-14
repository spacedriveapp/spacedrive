use sd_file_path_helper::{file_path_for_media_processor, IsolatedFilePathData};
use sd_prisma::prisma::{file_path, label, label_on_object, object, PrismaClient};
use sd_utils::{db::MissingFieldError, error::FileIOError};

use std::{
	collections::{HashMap, HashSet, VecDeque},
	path::{Path, PathBuf},
	sync::Arc,
};

use async_channel as chan;
use chrono::{DateTime, FixedOffset, Utc};
use futures_concurrency::future::{Join, Race};
use image::ImageFormat;
use tokio::{
	fs, spawn,
	sync::{oneshot, OwnedRwLockReadGuard, OwnedSemaphorePermit, RwLock, Semaphore},
	time::Instant,
};
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::{actor::Batch, model::ModelAndSession, BatchToken, ImageLabelerError, LabelerOutput};

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB

async fn reject_all_no_model(
	file_paths: Vec<file_path_for_media_processor::Data>,
	output_tx: &chan::Sender<LabelerOutput>,
) {
	file_paths
		.into_iter()
		.map(
			|file_path_for_media_processor::Data { id, .. }| async move {
				if output_tx
					.send(LabelerOutput {
						file_path_id: id,
						result: Err(ImageLabelerError::NoModelAvailable),
					})
					.await
					.is_err()
				{
					error!(
						"Failed to send batch output with no model error, <file_path_id='{id}'>"
					);
				}
			},
		)
		.collect::<Vec<_>>()
		.join()
		.await;
}

pub(super) enum FinishStatus {
	Interrupted(Batch),
	Done(BatchToken, chan::Sender<LabelerOutput>),
}

pub(super) async fn spawned_processing(
	model_and_session: Arc<RwLock<ModelAndSession>>,
	Batch {
		token,
		location_id,
		location_path,
		file_paths,
		output_tx,
		db,
		is_resumable,
	}: Batch,
	available_parallelism: usize,
	stop_rx: chan::Receiver<oneshot::Sender<()>>,
	done_tx: chan::Sender<FinishStatus>,
) {
	let mut errors = Vec::new();

	// We're already discarding failed ones, so we don't need to keep track of them
	let mut queue = file_paths
		.into_iter()
		.filter_map(|file_path| {
			if file_path.object_id.is_none() {
				errors.push((
					file_path.id,
					ImageLabelerError::IsolateFilePathData(MissingFieldError::new(
						"file_path.object_id",
					)),
				));

				return None;
			}

			let file_path_id = file_path.id;
			let Ok(iso_file_path) = IsolatedFilePathData::try_from((location_id, &file_path))
				.map_err(|e| {
					errors.push((file_path_id, e.into()));
				})
			else {
				return None;
			};

			match ImageFormat::from_extension(iso_file_path.extension()) {
				Some(format) => {
					let path = location_path.join(&iso_file_path);
					Some((file_path, path, format))
				}
				None => {
					errors.push((
						file_path_id,
						ImageLabelerError::UnsupportedExtension(
							file_path_id,
							iso_file_path.extension().to_owned(),
						),
					));

					None
				}
			}
		})
		.collect::<VecDeque<_>>();

	errors
		.into_iter()
		.map(|(file_path_id, error)| {
			let output_tx = &output_tx;
			async move {
				if output_tx
					.send(LabelerOutput {
						file_path_id,
						result: Err(error),
					})
					.await
					.is_err()
				{
					error!(
						"Failed to send batch output with errors, <file_path_id='{file_path_id}'>"
					);
				}
			}
		})
		.collect::<Vec<_>>()
		.join()
		.await;

	if queue.is_empty() {
		done_tx
			.send(FinishStatus::Done(token, output_tx))
			.await
			.expect("done_tx unexpectedly closed");
		return;
	}

	let semaphore = Arc::new(Semaphore::new(available_parallelism));

	// From this point ownwards, we lock the model in read mode
	let model_and_session = Arc::new(model_and_session.read_owned().await);

	if !model_and_session.can_process() {
		reject_all_no_model(
			queue
				.into_iter()
				.map(|(file_path, _, _)| file_path)
				.collect(),
			&output_tx,
		)
		.await;
		done_tx
			.send(FinishStatus::Done(token, output_tx))
			.await
			.expect("done_tx unexpectedly closed");
		return;
	}

	enum RaceOutput {
		Done,
		Stop(oneshot::Sender<()>),
	}

	let mut handles = Vec::with_capacity(queue.len());

	let mut on_flight = HashMap::with_capacity(queue.len());

	let (completed_tx, completex_rx) = chan::bounded(queue.len());

	let (finish_status, maybe_interrupted_tx) = if let RaceOutput::Stop(tx) = (
		async {
			while !queue.is_empty() {
				let (file_path, path, format) = queue.pop_front().expect("queue is not empty");

				let permit = Arc::clone(&semaphore)
					.acquire_owned()
					.await
					.expect("semaphore unexpectedly closed");

				let ids = (
					file_path.id,
					file_path.object_id.expect("alredy checked above"),
				);

				on_flight.insert(file_path.id, file_path);

				handles.push(spawn(spawned_process_single_file(
					Arc::clone(&model_and_session),
					ids,
					path,
					format,
					(output_tx.clone(), completed_tx.clone()),
					Arc::clone(&db),
					permit,
				)));
			}

			RaceOutput::Done
		},
		async { RaceOutput::Stop(stop_rx.recv().await.expect("stop_rx unexpectedly closed")) },
	)
		.race()
		.await
	{
		let start = Instant::now();
		for handle in &handles {
			handle.abort();
		}

		completed_tx.close();

		while let Ok(file_path_id) = completex_rx.recv().await {
			on_flight.remove(&file_path_id);
		}

		let status = if queue.is_empty() && on_flight.is_empty() {
			FinishStatus::Done(token, output_tx)
		} else {
			FinishStatus::Interrupted(Batch {
				token,
				location_id,
				location_path,
				file_paths: on_flight
					.into_values()
					.chain(queue.into_iter().map(|(file_path, _, _)| file_path))
					.collect(),
				output_tx,
				db,
				is_resumable,
			})
		};

		(status, Some(tx))
	} else {
		(FinishStatus::Done(token, output_tx), None)
	};

	if let Some(tx) = maybe_interrupted_tx {
		if let Err(e) = tx.send(()) {
			error!("Failed to send stop signal to image labeller batch processor: {e:#?}");
		}
	} else {
		handles
			.into_iter()
			.map(|handle| async move {
				if let Err(e) = handle.await {
					error!("Failed to join image labeller batch processor: {e:#?}");
				}
			})
			.collect::<Vec<_>>()
			.join()
			.await;
	}

	done_tx
		.send(finish_status)
		.await
		.expect("critical error: image labeller batch processor unexpectedly closed");
}

async fn spawned_process_single_file(
	model_and_session: Arc<OwnedRwLockReadGuard<ModelAndSession>>,
	(file_path_id, object_id): (file_path::id::Type, object::id::Type),
	path: PathBuf,
	format: ImageFormat,
	(output_tx, completed_tx): (
		chan::Sender<LabelerOutput>,
		chan::Sender<file_path::id::Type>,
	),
	db: Arc<PrismaClient>,
	_permit: OwnedSemaphorePermit,
) {
	let image =
		match extract_file_data(file_path_id, &path).await {
			Ok(image) => image,
			Err(e) => {
				if output_tx
					.send(LabelerOutput {
						file_path_id,
						result: Err(e),
					})
					.await
					.is_err()
				{
					error!("Failed to send batch output with I/O errors, <file_path_id='{file_path_id}'>");
				}

				if completed_tx.send(file_path_id).await.is_err() {
					warn!("Failed to send completed file path id, <file_path_id='{file_path_id}'>")
				}

				return;
			}
		};

	let labels = match model_and_session.process_single_image(path.as_path(), image, format) {
		Ok(labels) => labels,
		Err(e) => {
			if output_tx
				.send(LabelerOutput {
					file_path_id,
					result: Err(e),
				})
				.await
				.is_err()
			{
				error!("Failed to send batch output with model processing errors, <file_path_id='{file_path_id}'>");
			}

			if completed_tx.send(file_path_id).await.is_err() {
				warn!("Failed to send completed file path id, <file_path_id='{file_path_id}'>")
			}

			return;
		}
	};

	if output_tx
		.send(LabelerOutput {
			file_path_id,
			result: assign_labels(object_id, labels, &db).await,
		})
		.await
		.is_err()
	{
		error!("Failed to send batch output with database assign label results, <file_path_id='{file_path_id}'>");
	}

	if completed_tx.send(file_path_id).await.is_err() {
		warn!("Failed to send completed file path id, <file_path_id='{file_path_id}'>")
	}
}

async fn extract_file_data(
	file_path_id: file_path::id::Type,
	path: impl AsRef<Path>,
) -> Result<Vec<u8>, ImageLabelerError> {
	let path = path.as_ref();

	let metadata = fs::metadata(path).await.map_err(|e| {
		FileIOError::from((path, e, "Failed to get metadata for file to get labels"))
	})?;

	if metadata.len() > MAX_FILE_SIZE {
		return Err(ImageLabelerError::FileTooBig(
			file_path_id,
			metadata.len() as usize,
		));
	}

	fs::read(path)
		.await
		.map_err(|e| FileIOError::from((path, e, "Failed to read file to get labels")).into())
}

pub async fn assign_labels(
	object_id: object::id::Type,
	mut labels: HashSet<String>,
	db: &PrismaClient,
) -> Result<(), ImageLabelerError> {
	let mut labels_ids = db
		.label()
		.find_many(vec![label::name::in_vec(labels.iter().cloned().collect())])
		.select(label::select!({ id name }))
		.exec()
		.await?
		.into_iter()
		.map(|label| {
			labels.remove(&label.name);

			label.id
		})
		.collect::<Vec<_>>();

	labels_ids.reserve(labels.len());

	let date_created: DateTime<FixedOffset> = Utc::now().into();

	if !labels.is_empty() {
		labels_ids.extend(
			db._batch(
				labels
					.into_iter()
					.map(|name| {
						db.label()
							.create(
								Uuid::new_v4().as_bytes().to_vec(),
								name,
								vec![label::date_created::set(date_created)],
							)
							.select(label::select!({ id }))
					})
					.collect::<Vec<_>>(),
			)
			.await?
			.into_iter()
			.map(|label| label.id),
		);
	}

	db.label_on_object()
		.create_many(
			labels_ids
				.into_iter()
				.map(|label_id| {
					label_on_object::create_unchecked(
						label_id,
						object_id,
						vec![label_on_object::date_created::set(date_created)],
					)
				})
				.collect(),
		)
		.skip_duplicates()
		.exec()
		.await?;

	Ok(())
}
