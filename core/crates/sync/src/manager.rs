use sd_prisma::prisma::{cloud_crdt_operation, crdt_operation, instance, PrismaClient, SortOrder};
use sd_sync::{CRDTOperation, OperationFactory};
use sd_utils::{from_bytes_to_uuid, uuid_to_bytes};
use tracing::warn;

use std::{
	cmp, fmt,
	num::NonZeroU128,
	ops::Deref,
	sync::{
		atomic::{self, AtomicBool},
		Arc,
	},
};

use prisma_client_rust::{and, operator::or};
use tokio::sync::{broadcast, Mutex, Notify, RwLock};
use uhlc::{HLCBuilder, HLC};
use uuid::Uuid;

use super::{
	crdt_op_db,
	db_operation::{cloud_crdt_with_instance, crdt_with_instance},
	ingest, Error, SharedState, SyncMessage, NTP64,
};

/// Wrapper that spawns the ingest actor and provides utilities for reading and writing sync operations.
pub struct Manager {
	pub tx: broadcast::Sender<SyncMessage>,
	pub ingest: ingest::Handler,
	pub shared: Arc<SharedState>,
	pub timestamp_lock: Mutex<()>,
}

impl fmt::Debug for Manager {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("SyncManager").finish()
	}
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct GetOpsArgs {
	pub clocks: Vec<(Uuid, NTP64)>,
	pub count: u32,
}

impl Manager {
	/// Creates a new manager that can be used to read and write CRDT operations.
	/// Sync messages are received on the returned [`broadcast::Receiver<SyncMessage>`].
	pub async fn new(
		db: Arc<PrismaClient>,
		current_instance_uuid: Uuid,
		emit_messages_flag: Arc<AtomicBool>,
		actors: Arc<sd_actors::Actors>,
	) -> Result<(Self, broadcast::Receiver<SyncMessage>), Error> {
		let existing_instances = db.instance().find_many(vec![]).exec().await?;

		Self::with_existing_instances(
			db,
			current_instance_uuid,
			emit_messages_flag,
			&existing_instances,
			actors,
		)
		.await
	}

	/// Creates a new manager that can be used to read and write CRDT operations from a list of existing instances.
	/// Sync messages are received on the returned [`broadcast::Receiver<SyncMessage>`].
	///
	/// # Panics
	/// Panics if the `current_instance_id` UUID is zeroed.
	pub async fn with_existing_instances(
		db: Arc<PrismaClient>,
		current_instance_uuid: Uuid,
		emit_messages_flag: Arc<AtomicBool>,
		existing_instances: &[instance::Data],
		actors: Arc<sd_actors::Actors>,
	) -> Result<(Self, broadcast::Receiver<SyncMessage>), Error> {
		let timestamps = db
			._batch(
				existing_instances
					.iter()
					.map(|i| {
						db.crdt_operation()
							.find_first(vec![crdt_operation::instance::is(vec![
								instance::id::equals(i.id),
							])])
							.order_by(crdt_operation::timestamp::order(SortOrder::Desc))
					})
					.collect::<Vec<_>>(),
			)
			.await?
			.into_iter()
			.zip(existing_instances)
			.map(|(op, i)| {
				(
					from_bytes_to_uuid(&i.pub_id),
					#[allow(clippy::cast_sign_loss)]
					// SAFETY: we had to store using i64 due to SQLite limitations
					NTP64(op.map(|o| o.timestamp).unwrap_or_default() as u64),
				)
			})
			.collect();

		let (tx, rx) = broadcast::channel(64);

		let clock = HLCBuilder::new()
			.with_id(uhlc::ID::from(
				NonZeroU128::new(current_instance_uuid.to_u128_le()).expect("Non zero id"),
			))
			.build();

		let shared = Arc::new(SharedState {
			db,
			instance: current_instance_uuid,
			clock,
			timestamps: Arc::new(RwLock::new(timestamps)),
			emit_messages_flag,
			active: AtomicBool::default(),
			active_notify: Notify::default(),
			actors,
		});

		let ingest = ingest::Actor::declare(shared.clone()).await;

		Ok((
			Self {
				tx,
				ingest,
				shared,
				timestamp_lock: Mutex::default(),
			},
			rx,
		))
	}

	pub fn subscribe(&self) -> broadcast::Receiver<SyncMessage> {
		self.tx.subscribe()
	}

	pub async fn write_ops<'item, Q>(
		&self,
		tx: &PrismaClient,
		(mut ops, queries): (Vec<CRDTOperation>, Q),
	) -> Result<Q::ReturnValue, Error>
	where
		Q: prisma_client_rust::BatchItem<'item, ReturnValue: Send> + Send,
	{
		let ret = if self.emit_messages_flag.load(atomic::Ordering::Relaxed) {
			let lock = self.timestamp_lock.lock().await;

			for op in &mut ops {
				op.timestamp = *self.get_clock().new_timestamp().get_time();
			}

			let (res, _) = tx
				._batch((
					queries,
					ops.iter()
						.map(|op| crdt_op_db(op).map(|q| q.to_query(tx)))
						.collect::<Result<Vec<_>, _>>()?,
				))
				.await?;

			if let Some(last) = ops.last() {
				self.shared
					.timestamps
					.write()
					.await
					.insert(self.instance, last.timestamp);
			}

			if self.tx.send(SyncMessage::Created).is_err() {
				warn!("failed to send created message on `write_ops`");
			}

			drop(lock);

			res
		} else {
			tx._batch([queries]).await?.remove(0)
		};

		Ok(ret)
	}

	pub async fn write_op<'item, Q>(
		&self,
		tx: &PrismaClient,
		mut op: CRDTOperation,
		query: Q,
	) -> Result<Q::ReturnValue, Error>
	where
		Q: prisma_client_rust::BatchItem<'item, ReturnValue: Send> + Send,
	{
		let ret = if self.emit_messages_flag.load(atomic::Ordering::Relaxed) {
			let lock = self.timestamp_lock.lock().await;

			op.timestamp = *self.get_clock().new_timestamp().get_time();

			let ret = tx._batch((crdt_op_db(&op)?.to_query(tx), query)).await?.1;

			if self.tx.send(SyncMessage::Created).is_err() {
				warn!("failed to send created message on `write_op`");
			}

			drop(lock);

			ret
		} else {
			tx._batch(vec![query]).await?.remove(0)
		};

		self.shared
			.timestamps
			.write()
			.await
			.insert(self.instance, op.timestamp);

		Ok(ret)
	}

	pub async fn get_instance_ops(
		&self,
		count: u32,
		instance_uuid: Uuid,
		timestamp: NTP64,
	) -> Result<Vec<CRDTOperation>, Error> {
		self.db
			.crdt_operation()
			.find_many(vec![
				crdt_operation::instance::is(vec![instance::pub_id::equals(uuid_to_bytes(
					&instance_uuid,
				))]),
				#[allow(clippy::cast_possible_wrap)]
				crdt_operation::timestamp::gt(timestamp.as_u64() as i64),
			])
			.take(i64::from(count))
			.order_by(crdt_operation::timestamp::order(SortOrder::Asc))
			.include(crdt_with_instance::include())
			.exec()
			.await?
			.into_iter()
			.map(crdt_with_instance::Data::into_operation)
			.collect()
	}

	pub async fn get_ops(&self, args: GetOpsArgs) -> Result<Vec<CRDTOperation>, Error> {
		let mut ops = self
			.db
			.crdt_operation()
			.find_many(vec![or(args
				.clocks
				.iter()
				.map(|(instance_id, timestamp)| {
					and![
						crdt_operation::instance::is(vec![instance::pub_id::equals(
							uuid_to_bytes(instance_id)
						)]),
						crdt_operation::timestamp::gt({
							#[allow(clippy::cast_possible_wrap)]
							// SAFETY: we had to store using i64 due to SQLite limitations
							{
								timestamp.as_u64() as i64
							}
						})
					]
				})
				.chain([crdt_operation::instance::is_not(vec![
					instance::pub_id::in_vec(
						args.clocks
							.iter()
							.map(|(instance_id, _)| uuid_to_bytes(instance_id))
							.collect(),
					),
				])])
				.collect())])
			.take(i64::from(args.count))
			.order_by(crdt_operation::timestamp::order(SortOrder::Asc))
			.include(crdt_with_instance::include())
			.exec()
			.await?;

		ops.sort_by(|a, b| match a.timestamp().cmp(&b.timestamp()) {
			cmp::Ordering::Equal => a.instance().cmp(&b.instance()),
			o => o,
		});

		ops.into_iter()
			.take(args.count as usize)
			.map(crdt_with_instance::Data::into_operation)
			.collect()
	}

	pub async fn get_cloud_ops(
		&self,
		args: GetOpsArgs,
	) -> Result<Vec<(i32, CRDTOperation)>, Error> {
		let mut ops = self
			.db
			.cloud_crdt_operation()
			.find_many(vec![or(args
				.clocks
				.iter()
				.map(|(instance_id, timestamp)| {
					and![
						cloud_crdt_operation::instance::is(vec![instance::pub_id::equals(
							uuid_to_bytes(instance_id)
						)]),
						cloud_crdt_operation::timestamp::gt({
							#[allow(clippy::cast_possible_wrap)]
							// SAFETY: we had to store using i64 due to SQLite limitations
							{
								timestamp.as_u64() as i64
							}
						})
					]
				})
				.chain([cloud_crdt_operation::instance::is_not(vec![
					instance::pub_id::in_vec(
						args.clocks
							.iter()
							.map(|(instance_id, _)| uuid_to_bytes(instance_id))
							.collect(),
					),
				])])
				.collect())])
			.take(i64::from(args.count))
			.order_by(cloud_crdt_operation::timestamp::order(SortOrder::Asc))
			.include(cloud_crdt_with_instance::include())
			.exec()
			.await?;

		ops.sort_by(|a, b| match a.timestamp().cmp(&b.timestamp()) {
			cmp::Ordering::Equal => a.instance().cmp(&b.instance()),
			o => o,
		});

		ops.into_iter()
			.take(args.count as usize)
			.map(cloud_crdt_with_instance::Data::into_operation)
			.collect()
	}
}

impl OperationFactory for Manager {
	fn get_clock(&self) -> &HLC {
		&self.clock
	}

	fn get_instance(&self) -> Uuid {
		self.instance
	}
}

impl Deref for Manager {
	type Target = SharedState;

	fn deref(&self) -> &Self::Target {
		&self.shared
	}
}
