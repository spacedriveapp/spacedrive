use sd_core_file_path_helper::IsolatedFilePathData;
use sd_core_prisma_helpers::file_path_for_media_processor;
use sd_core_sync::SyncManager;

use sd_prisma::{
	prisma::{device, file_path, label, label_on_object, object, PrismaClient},
	prisma_sync,
};
use sd_sync::OperationFactory;
use sd_utils::{db::MissingFieldError, error::FileIOError, msgpack};

use std::{
	collections::{BTreeMap, HashMap, HashSet, VecDeque},
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
};
use tracing::{error, warn};

use super::{
	model::ModelAndSession, old_actor::Batch, BatchToken, ImageLabelerError, LabelerOutput,
};

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
						has_new_labels: false,
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
		device_id,
		file_paths,
		output_tx,
		db,
		sync,
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
			if file_path.object.is_none() {
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
						has_new_labels: false,
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

	// From this point onwards, we lock the model in read mode
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

	let (completed_tx, completed_rx) = chan::bounded(queue.len());

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
					file_path.object.as_ref().expect("already checked above").id,
					device_id,
				);

				if output_tx.is_closed() {
					warn!("Image labeler output channel was closed, dropping current batch...");
					queue.clear();
					on_flight.clear();

					break;
				}

				on_flight.insert(file_path.id, file_path);

				handles.push(spawn(spawned_process_single_file(
					Arc::clone(&model_and_session),
					ids,
					path,
					format,
					(output_tx.clone(), completed_tx.clone()),
					Arc::clone(&db),
					sync.clone(),
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
		for handle in &handles {
			handle.abort();
		}

		completed_tx.close();

		while let Ok(file_path_id) = completed_rx.recv().await {
			on_flight.remove(&file_path_id);
		}

		let status = if queue.is_empty() && on_flight.is_empty() {
			FinishStatus::Done(token, output_tx)
		} else {
			FinishStatus::Interrupted(Batch {
				token,
				location_id,
				location_path,
				device_id,
				file_paths: on_flight
					.into_values()
					.chain(queue.into_iter().map(|(file_path, _, _)| file_path))
					.collect(),
				output_tx,
				db,
				sync: sync.clone(),
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

#[allow(clippy::too_many_arguments)]
async fn spawned_process_single_file(
	model_and_session: Arc<OwnedRwLockReadGuard<ModelAndSession>>,
	(file_path_id, object_id, device_id): (file_path::id::Type, object::id::Type, device::id::Type),
	path: PathBuf,
	format: ImageFormat,
	(output_tx, completed_tx): (
		chan::Sender<LabelerOutput>,
		chan::Sender<file_path::id::Type>,
	),
	db: Arc<PrismaClient>,
	sync: SyncManager,
	_permit: OwnedSemaphorePermit,
) {
	let image =
		match extract_file_data(file_path_id, &path).await {
			Ok(image) => image,
			Err(e) => {
				if output_tx
					.send(LabelerOutput {
						file_path_id,
						has_new_labels: false,
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
					has_new_labels: false,
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

	let (has_new_labels, result) =
		match assign_labels(object_id, device_id, labels, &db, &sync).await {
			Ok(has_new_labels) => (has_new_labels, Ok(())),
			Err(e) => (false, Err(e)),
		};

	if output_tx
		.send(LabelerOutput {
			file_path_id,
			has_new_labels,
			result,
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
	device_id: device::id::Type,
	mut labels: HashSet<String>,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<bool, ImageLabelerError> {
	let object = db
		.object()
		.find_unique(object::id::equals(object_id))
		.select(object::select!({ pub_id }))
		.exec()
		.await?
		.unwrap();

	let mut has_new_labels = false;

	let mut labels_ids = db
		.label()
		.find_many(vec![label::name::in_vec(labels.iter().cloned().collect())])
		.select(label::select!({ id name }))
		.exec()
		.await?
		.into_iter()
		.map(|label| {
			labels.remove(&label.name);

			(label.id, label.name)
		})
		.collect::<BTreeMap<_, _>>();

	let date_created: DateTime<FixedOffset> = Utc::now().into();

	if !labels.is_empty() {
		let mut sync_params = Vec::with_capacity(labels.len() * 2);

		let db_params = labels
			.into_iter()
			.map(|name| {
				sync_params.push(sync.shared_create(
					prisma_sync::label::SyncId { name: name.clone() },
					[(label::date_created::NAME, msgpack!(&date_created))],
				));

				db.label()
					.create(name, vec![label::date_created::set(Some(date_created))])
					.select(label::select!({ id name }))
			})
			.collect::<Vec<_>>();

		labels_ids.extend(
			sync.write_ops(db, (sync_params, db_params))
				.await?
				.into_iter()
				.map(|l| (l.id, l.name)),
		);

		has_new_labels = true;
	}

	let mut sync_params = Vec::with_capacity(labels_ids.len() * 2);

	if !labels_ids.is_empty() {
		let db_params: Vec<_> = labels_ids
			.into_iter()
			.map(|(label_id, name)| {
				sync_params.push(sync.relation_create(
					prisma_sync::label_on_object::SyncId {
						label: prisma_sync::label::SyncId { name },
						object: prisma_sync::object::SyncId {
							pub_id: object.pub_id.clone(),
						},
					},
					[(
						label_on_object::device::NAME,
						msgpack!(prisma_sync::device::SyncId {
							pub_id: sync.device_pub_id.to_db(),
						}),
					)],
				));

				label_on_object::create_unchecked(
					label_id,
					object_id,
					vec![
						label_on_object::date_created::set(date_created),
						label_on_object::device_id::set(Some(device_id)),
					],
				)
			})
			.collect();

		sync.write_ops(
			db,
			(
				sync_params,
				db.label_on_object()
					.create_many(db_params)
					.skip_duplicates(),
			),
		)
		.await?;
	}

	Ok(has_new_labels)
}
