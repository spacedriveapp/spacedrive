use sd_core_prisma_helpers::DevicePubId;

use sd_prisma::{
	prisma::{crdt_operation, device, PrismaClient, SortOrder},
	prisma_sync,
};
use sd_sync::{
	CRDTOperation, CompressedCRDTOperationsPerModel, CompressedCRDTOperationsPerModelPerDevice,
	ModelId, OperationFactory,
};
use sd_utils::timestamp_to_datetime;

use std::{
	collections::BTreeMap,
	fmt,
	num::NonZeroU128,
	sync::{
		atomic::{self, AtomicBool},
		Arc,
	},
};

use async_stream::stream;
use futures::Stream;
use futures_concurrency::future::TryJoin;
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use tracing::{debug, warn};
use uhlc::{HLCBuilder, HLC};
use uuid::Uuid;

use super::{
	crdt_op_db, db_operation::from_crdt_ops, ingest_utils::process_crdt_operations, Error,
	SyncEvent, TimestampPerDevice, NTP64,
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
	pub sync_lock: Arc<Mutex<()>>,
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
			},
			rx,
		))
	}

	pub async fn ingest_ops(
		&self,
		CompressedCRDTOperationsPerModelPerDevice(compressed_ops): CompressedCRDTOperationsPerModelPerDevice,
	) -> Result<(), Error> {
		// WARN: this order here exists because sync messages MUST be processed in this exact order
		// due to relationship dependencies between these tables.
		const INGEST_ORDER: &[ModelId] = &[
			prisma_sync::device::MODEL_ID,
			prisma_sync::storage_statistics::MODEL_ID,
			prisma_sync::tag::MODEL_ID,
			prisma_sync::location::MODEL_ID,
			prisma_sync::object::MODEL_ID,
			prisma_sync::exif_data::MODEL_ID,
			prisma_sync::file_path::MODEL_ID,
			prisma_sync::label::MODEL_ID,
			prisma_sync::tag_on_object::MODEL_ID,
			prisma_sync::label_on_object::MODEL_ID,
		];

		let _lock_guard = self.sync_lock.lock().await;

		let mut ops_fut_by_model = INGEST_ORDER
			.iter()
			.map(|&model_id| (model_id, vec![]))
			.collect::<BTreeMap<_, _>>();

		for (device_pub_id, CompressedCRDTOperationsPerModel(ops_per_model)) in compressed_ops {
			for (model_id, ops_per_record) in ops_per_model {
				for (record_id, ops) in ops_per_record {
					ops_fut_by_model
						.get_mut(&model_id)
						.ok_or(Error::InvalidModelId(model_id))?
						.push(process_crdt_operations(
							&self.clock,
							&self.timestamp_per_device,
							&self.db,
							device_pub_id.into(),
							model_id,
							record_id,
							ops,
						));
				}
			}
		}

		for model_id in INGEST_ORDER {
			if let Some(futs) = ops_fut_by_model.remove(model_id) {
				futs.try_join().await?;
			}
		}

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
