use sd_core_prisma_helpers::DevicePubId;

use sd_prisma::{
	prisma::{cloud_crdt_operation, crdt_operation, device, PrismaClient, SortOrder},
	prisma_sync,
};
use sd_sync::{CRDTOperation, CompressedCRDTOperation, ModelId, OperationFactory, RecordId};
use sd_utils::timestamp_to_datetime;

use std::{
	collections::{hash_map::Entry, BTreeMap, HashMap},
	fmt,
	num::NonZeroU128,
	sync::{
		atomic::{self, AtomicBool},
		Arc,
	},
	time::SystemTime,
};

use async_stream::stream;
use futures::Stream;
use futures_concurrency::future::{Join, TryJoin};
use tokio::{
	spawn,
	sync::{broadcast, Mutex, Notify, RwLock, Semaphore},
};
use tracing::{debug, instrument, warn};
use uhlc::{HLCBuilder, HLC};
use uuid::Uuid;

use super::{
	crdt_op_db,
	db_operation::{from_cloud_crdt_ops, from_crdt_ops},
	ingest_utils::process_crdt_operations,
	Error, SyncEvent, TimestampPerDevice, NTP64,
};

/// Wrapper that spawns the ingest actor and provides utilities for reading and writing sync operations.
#[derive(Clone)]
pub struct Manager {
	pub tx: broadcast::Sender<SyncEvent>,
	pub db: Arc<PrismaClient>,
	pub emit_messages_flag: Arc<AtomicBool>,
	pub device_pub_id: DevicePubId,
	pub timestamp_per_device: TimestampPerDevice,
	pub clock: Arc<HLC>,
	pub active: Arc<AtomicBool>,
	pub active_notify: Arc<Notify>,
	pub(crate) sync_lock: Arc<Mutex<()>>,
	pub(crate) available_parallelism: usize,
}

impl fmt::Debug for Manager {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("SyncManager").finish()
	}
}

impl Manager {
	/// Creates a new manager that can be used to read and write CRDT operations.
	/// Sync messages are received on the returned [`broadcast::Receiver<SyncMessage>`].
	pub async fn new(
		db: Arc<PrismaClient>,
		current_device_pub_id: &DevicePubId,
		emit_messages_flag: Arc<AtomicBool>,
	) -> Result<(Self, broadcast::Receiver<SyncEvent>), Error> {
		let existing_devices = db.device().find_many(vec![]).exec().await?;

		Self::with_existing_devices(
			db,
			current_device_pub_id,
			emit_messages_flag,
			&existing_devices,
		)
		.await
	}

	/// Creates a new manager that can be used to read and write CRDT operations from a list of existing instances.
	/// Sync messages are received on the returned [`broadcast::Receiver<SyncMessage>`].
	///
	/// # Panics
	/// Panics if the `current_device_pub_id` UUID is zeroed, which will never happen as we use `UUIDv7` for the
	/// device pub id. As this version have a timestamp part, instead of being totally random. So the only
	/// possible way to get zero from a `UUIDv7` is to go back in time to 1970
	pub async fn with_existing_devices(
		db: Arc<PrismaClient>,
		current_device_pub_id: &DevicePubId,
		emit_messages_flag: Arc<AtomicBool>,
		existing_devices: &[device::Data],
	) -> Result<(Self, broadcast::Receiver<SyncEvent>), Error> {
		let latest_timestamp_per_device = db
			._batch(
				existing_devices
					.iter()
					.map(|device| {
						db.crdt_operation()
							.find_first(vec![crdt_operation::device_pub_id::equals(
								device.pub_id.clone(),
							)])
							.order_by(crdt_operation::timestamp::order(SortOrder::Desc))
					})
					.collect::<Vec<_>>(),
			)
			.await?
			.into_iter()
			.zip(existing_devices)
			.map(|(op, device)| {
				(
					DevicePubId::from(&device.pub_id),
					#[allow(clippy::cast_sign_loss)]
					// SAFETY: we had to store using i64 due to SQLite limitations
					NTP64(op.map(|o| o.timestamp).unwrap_or_default() as u64),
				)
			})
			.collect();

		let (tx, rx) = broadcast::channel(64);

		Ok((
			Self {
				tx,
				db,
				device_pub_id: current_device_pub_id.clone(),
				clock: Arc::new(
					HLCBuilder::new()
						.with_id(uhlc::ID::from(
							NonZeroU128::new(Uuid::from(current_device_pub_id).to_u128_le())
								.expect("Non zero id"),
						))
						.build(),
				),
				timestamp_per_device: Arc::new(RwLock::new(latest_timestamp_per_device)),
				emit_messages_flag,
				active: Arc::default(),
				active_notify: Arc::default(),
				sync_lock: Arc::new(Mutex::default()),
				available_parallelism: std::thread::available_parallelism()
					.map_or(1, std::num::NonZero::get),
			},
			rx,
		))
	}

	async fn fetch_cloud_crdt_ops(
		&self,
		model_id: ModelId,
		batch_size: i64,
	) -> Result<(Vec<cloud_crdt_operation::id::Type>, Vec<CRDTOperation>), Error> {
		self.db
			.cloud_crdt_operation()
			.find_many(vec![cloud_crdt_operation::model::equals(i32::from(
				model_id,
			))])
			.take(batch_size)
			.order_by(cloud_crdt_operation::timestamp::order(SortOrder::Asc))
			.exec()
			.await?
			.into_iter()
			.map(from_cloud_crdt_ops)
			.collect::<Result<(Vec<_>, Vec<_>), _>>()
	}

	#[instrument(skip(self))]
	async fn ingest_by_model(&self, model_id: ModelId) -> Result<(), Error> {
		let mut total_count = 0;

		loop {
			let (ops_ids, ops) = self.fetch_cloud_crdt_ops(model_id, 10_000).await?;
			if ops_ids.is_empty() {
				break;
			}

			debug!(
				messages_count = ops.len(),
				first_message = ?ops
						.first()
						.map_or_else(|| SystemTime::UNIX_EPOCH.into(), |op| timestamp_to_datetime(op.timestamp)),
				last_message = ?ops
						.last()
						.map_or_else(|| SystemTime::UNIX_EPOCH.into(), |op| timestamp_to_datetime(op.timestamp)),
				"Messages by model to ingest",
			);

			let mut compressed_map =
				BTreeMap::<Uuid, HashMap<Vec<u8>, (RecordId, Vec<CompressedCRDTOperation>)>>::new();

			for CRDTOperation {
				device_pub_id,
				timestamp,
				model_id: _, // Ignoring model_id as we know it already
				record_id,
				data,
			} in ops
			{
				let records = compressed_map.entry(device_pub_id).or_default();

				// Can't use RecordId as a key because rmpv::Value doesn't implement Hash + Eq.
				// So we use it's serialized bytes as a key.
				let record_id_bytes =
					rmp_serde::to_vec_named(&record_id).expect("already serialized to Value");

				match records.entry(record_id_bytes) {
					Entry::Occupied(mut entry) => {
						entry
							.get_mut()
							.1
							.push(CompressedCRDTOperation { timestamp, data });
					}
					Entry::Vacant(entry) => {
						entry
							.insert((record_id, vec![CompressedCRDTOperation { timestamp, data }]));
					}
				}
			}

			let _lock_guard = self.sync_lock.lock().await;

			let semaphore = &Arc::new(Semaphore::new(self.available_parallelism));

			let handles = compressed_map
				.into_iter()
				.flat_map(|(device_pub_id, records)| {
					records.into_values().map(move |(record_id, ops)| {
						// We can process each record in parallel as they are independent
						spawn({
							let clock = Arc::clone(&self.clock);
							let timestamp_per_device = Arc::clone(&self.timestamp_per_device);
							let db = Arc::clone(&self.db);
							let device_pub_id = device_pub_id.into();
							let semaphore = Arc::clone(semaphore);

							async move {
								let _permit =
									semaphore.acquire().await.expect("semaphore never closes");

								let count = ops.len();

								process_crdt_operations(
									&clock,
									&timestamp_per_device,
									&db,
									device_pub_id,
									model_id,
									record_id,
									ops,
								)
								.await
								.map(|()| count)
							}
						})
					})
				})
				.collect::<Vec<_>>();

			for res in handles.join().await {
				let count = res.map_err(Error::ProcessCrdtPanic)??;
				debug!(count, "Ingested operations of model");
				total_count += count;
			}

			self.db
				.cloud_crdt_operation()
				.delete_many(vec![cloud_crdt_operation::id::in_vec(ops_ids)])
				.exec()
				.await?;
		}

		debug!(total_count, "Ingested all operations of this model");

		Ok(())
	}

	pub async fn ingest_ops(&self) -> Result<(), Error> {
		// WARN: this order here exists because sync messages MUST be processed in this exact order
		// due to relationship dependencies between these tables.
		self.ingest_by_model(prisma_sync::device::MODEL_ID).await?;

		(
			self.ingest_by_model(prisma_sync::storage_statistics::MODEL_ID),
			self.ingest_by_model(prisma_sync::tag::MODEL_ID),
			self.ingest_by_model(prisma_sync::location::MODEL_ID),
			self.ingest_by_model(prisma_sync::object::MODEL_ID),
			self.ingest_by_model(prisma_sync::label::MODEL_ID),
		)
			.try_join()
			.await?;

		(
			self.ingest_by_model(prisma_sync::exif_data::MODEL_ID),
			self.ingest_by_model(prisma_sync::file_path::MODEL_ID),
			self.ingest_by_model(prisma_sync::tag_on_object::MODEL_ID),
			self.ingest_by_model(prisma_sync::label_on_object::MODEL_ID),
		)
			.try_join()
			.await?;

		if self.tx.send(SyncEvent::Ingested).is_err() {
			warn!("failed to send ingested message on `ingest_ops`");
		}

		Ok(())
	}

	#[must_use]
	pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
		self.tx.subscribe()
	}

	pub async fn write_ops<'item, Q>(
		&self,
		tx: &PrismaClient,
		(ops, queries): (Vec<CRDTOperation>, Q),
	) -> Result<Q::ReturnValue, Error>
	where
		Q: prisma_client_rust::BatchItem<'item, ReturnValue: Send> + Send,
	{
		if ops.is_empty() {
			return Err(Error::EmptyOperations);
		}

		let ret = if self.emit_messages_flag.load(atomic::Ordering::Relaxed) {
			let lock_guard = self.sync_lock.lock().await;

			let (res, _) = tx
				._batch((
					queries,
					ops.iter()
						.map(|op| crdt_op_db(op).map(|q| q.to_query(tx)))
						.collect::<Result<Vec<_>, _>>()?,
				))
				.await?;

			if let Some(last) = ops.last() {
				self.timestamp_per_device
					.write()
					.await
					.insert(self.device_pub_id.clone(), last.timestamp);
			}

			if self.tx.send(SyncEvent::Created).is_err() {
				warn!("failed to send created message on `write_ops`");
			}

			drop(lock_guard);

			res
		} else {
			tx._batch([queries]).await?.remove(0)
		};

		Ok(ret)
	}

	pub async fn write_op<'item, Q>(
		&self,
		tx: &PrismaClient,
		op: CRDTOperation,
		query: Q,
	) -> Result<Q::ReturnValue, Error>
	where
		Q: prisma_client_rust::BatchItem<'item, ReturnValue: Send> + Send,
	{
		let ret = if self.emit_messages_flag.load(atomic::Ordering::Relaxed) {
			let lock_guard = self.sync_lock.lock().await;

			let ret = tx._batch((crdt_op_db(&op)?.to_query(tx), query)).await?.1;

			if self.tx.send(SyncEvent::Created).is_err() {
				warn!("failed to send created message on `write_op`");
			}

			drop(lock_guard);

			ret
		} else {
			tx._batch(vec![query]).await?.remove(0)
		};

		self.timestamp_per_device
			.write()
			.await
			.insert(self.device_pub_id.clone(), op.timestamp);

		Ok(ret)
	}

	// pub async fn get_device_ops(
	// 	&self,
	// 	count: u32,
	// 	device_pub_id: DevicePubId,
	// 	timestamp: NTP64,
	// ) -> Result<Vec<CRDTOperation>, Error> {
	// 	self.db
	// 		.crdt_operation()
	// 		.find_many(vec![
	// 			crdt_operation::device_pub_id::equals(device_pub_id.into()),
	// 			#[allow(clippy::cast_possible_wrap)]
	// 			crdt_operation::timestamp::gt(timestamp.as_u64() as i64),
	// 		])
	// 		.take(i64::from(count))
	// 		.order_by(crdt_operation::timestamp::order(SortOrder::Asc))
	// 		.exec()
	// 		.await?
	// 		.into_iter()
	// 		.map(from_crdt_ops)
	// 		.collect()
	// }

	pub fn stream_device_ops<'a>(
		&'a self,
		device_pub_id: &'a DevicePubId,
		chunk_size: u32,
		initial_timestamp: NTP64,
	) -> impl Stream<Item = Result<Vec<CRDTOperation>, Error>> + Send + 'a {
		stream! {
			let mut current_initial_timestamp = initial_timestamp;

			loop {
				match self.db.crdt_operation()
					.find_many(vec![
						crdt_operation::device_pub_id::equals(device_pub_id.to_db()),
						#[allow(clippy::cast_possible_wrap)]
						crdt_operation::timestamp::gt(current_initial_timestamp.as_u64() as i64),
					])
					.take(i64::from(chunk_size))
					.order_by(crdt_operation::timestamp::order(SortOrder::Asc))
					.exec()
					.await
				{
					Ok(ops) if ops.is_empty() => break,

					Ok(ops) => match ops
						.into_iter()
						.map(from_crdt_ops)
						.collect::<Result<Vec<_>, _>>()
					{
						Ok(ops) => {
							debug!(
								start_datetime = ?ops
									.first()
									.map(|op| timestamp_to_datetime(op.timestamp)),
								end_datetime = ?ops
									.last()
									.map(|op| timestamp_to_datetime(op.timestamp)),
								count = ops.len(),
								"Streaming crdt ops",
							);

							if let Some(last_op) = ops.last() {
								current_initial_timestamp = last_op.timestamp;
							}

							yield Ok(ops);
						}

						Err(e) => return yield Err(e),
					}

					Err(e) => return yield Err(e.into())
				}
			}
		}
	}

	// pub async fn get_ops(
	// 	&self,
	// 	count: u32,
	// 	timestamp_per_device: Vec<(DevicePubId, NTP64)>,
	// ) -> Result<Vec<CRDTOperation>, Error> {
	// 	let mut ops = self
	// 		.db
	// 		.crdt_operation()
	// 		.find_many(vec![or(timestamp_per_device
	// 			.iter()
	// 			.map(|(device_pub_id, timestamp)| {
	// 				and![
	// 					crdt_operation::device_pub_id::equals(device_pub_id.to_db()),
	// 					crdt_operation::timestamp::gt({
	// 						#[allow(clippy::cast_possible_wrap)]
	// 						// SAFETY: we had to store using i64 due to SQLite limitations
	// 						{
	// 							timestamp.as_u64() as i64
	// 						}
	// 					})
	// 				]
	// 			})
	// 			.chain([crdt_operation::device_pub_id::not_in_vec(
	// 				timestamp_per_device
	// 					.iter()
	// 					.map(|(device_pub_id, _)| device_pub_id.to_db())
	// 					.collect(),
	// 			)])
	// 			.collect())])
	// 		.take(i64::from(count))
	// 		.order_by(crdt_operation::timestamp::order(SortOrder::Asc))
	// 		.exec()
	// 		.await?;

	// 	ops.sort_by(|a, b| match a.timestamp.cmp(&b.timestamp) {
	// 		cmp::Ordering::Equal => {
	// 			from_bytes_to_uuid(&a.device_pub_id).cmp(&from_bytes_to_uuid(&b.device_pub_id))
	// 		}
	// 		o => o,
	// 	});

	// 	ops.into_iter()
	// 		.take(count as usize)
	// 		.map(from_crdt_ops)
	// 		.collect()
	// }

	// pub async fn get_cloud_ops(
	// 	&self,
	// 	count: u32,
	// 	timestamp_per_device: Vec<(DevicePubId, NTP64)>,
	// ) -> Result<Vec<(cloud_crdt_operation::id::Type, CRDTOperation)>, Error> {
	// 	let mut ops = self
	// 		.db
	// 		.cloud_crdt_operation()
	// 		.find_many(vec![or(timestamp_per_device
	// 			.iter()
	// 			.map(|(device_pub_id, timestamp)| {
	// 				and![
	// 					cloud_crdt_operation::device_pub_id::equals(device_pub_id.to_db()),
	// 					cloud_crdt_operation::timestamp::gt({
	// 						#[allow(clippy::cast_possible_wrap)]
	// 						// SAFETY: we had to store using i64 due to SQLite limitations
	// 						{
	// 							timestamp.as_u64() as i64
	// 						}
	// 					})
	// 				]
	// 			})
	// 			.chain([cloud_crdt_operation::device_pub_id::not_in_vec(
	// 				timestamp_per_device
	// 					.iter()
	// 					.map(|(device_pub_id, _)| device_pub_id.to_db())
	// 					.collect(),
	// 			)])
	// 			.collect())])
	// 		.take(i64::from(count))
	// 		.order_by(cloud_crdt_operation::timestamp::order(SortOrder::Asc))
	// 		.exec()
	// 		.await?;

	// 	ops.sort_by(|a, b| match a.timestamp.cmp(&b.timestamp) {
	// 		cmp::Ordering::Equal => {
	// 			from_bytes_to_uuid(&a.device_pub_id).cmp(&from_bytes_to_uuid(&b.device_pub_id))
	// 		}
	// 		o => o,
	// 	});

	// 	ops.into_iter()
	// 		.take(count as usize)
	// 		.map(from_cloud_crdt_ops)
	// 		.collect()
	// }
}

impl OperationFactory for Manager {
	fn get_clock(&self) -> &HLC {
		&self.clock
	}

	fn get_device_pub_id(&self) -> sd_sync::DevicePubId {
		sd_sync::DevicePubId::from(&self.device_pub_id)
	}
}
