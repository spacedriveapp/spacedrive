use sd_core_prisma_helpers::DevicePubId;

use sd_prisma::{
	prisma::{crdt_operation, PrismaClient, SortOrder},
	prisma_sync::ModelSyncData,
};
use sd_sync::{
	CRDTOperation, CRDTOperationData, CompressedCRDTOperation, ModelId, OperationKind, RecordId,
};

use std::{collections::BTreeMap, num::NonZeroU128};

use futures_concurrency::future::TryJoin;
use tracing::{debug, instrument, trace, warn};
use uhlc::{Timestamp, HLC, NTP64};
use uuid::Uuid;

use super::{db_operation::write_crdt_op_to_db, Error, TimestampPerDevice};

// where the magic happens
#[instrument(skip(clock, ops), fields(operations_count = %ops.len()), err)]
pub async fn process_crdt_operations(
	clock: &HLC,
	timestamp_per_device: &TimestampPerDevice,
	db: &PrismaClient,
	device_pub_id: DevicePubId,
	model: ModelId,
	record_id: RecordId,
	mut ops: Vec<CompressedCRDTOperation>,
) -> Result<(), Error> {
	ops.sort_by_key(|op| op.timestamp);

	let new_timestamp = ops.last().expect("Empty ops array").timestamp;

	// first, we update the HLC's timestamp with the incoming one.
	// this involves a drift check + sets the last time of the clock
	clock
		.update_with_timestamp(&Timestamp::new(
			new_timestamp,
			uhlc::ID::from(
				NonZeroU128::new(Uuid::from(&device_pub_id).to_u128_le()).expect("Non zero id"),
			),
		))
		.expect("timestamp has too much drift!");

	// Delete - ignores all other messages
	if let Some(delete_op) = ops
		.iter()
		.rev()
		.find(|op| matches!(op.data, CRDTOperationData::Delete))
	{
		trace!("Deleting operation");
		handle_crdt_deletion(db, &device_pub_id, model, record_id, delete_op).await?;
	}
	// Create + > 0 Update - overwrites the create's data with the updates
	else if let Some(timestamp) = ops
		.iter()
		.rev()
		.find_map(|op| matches!(&op.data, CRDTOperationData::Create(_)).then_some(op.timestamp))
	{
		trace!("Create + Updates operations");

		// conflict resolution
		let delete = db
			.crdt_operation()
			.find_first(vec![
				crdt_operation::model::equals(i32::from(model)),
				crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
				crdt_operation::kind::equals(OperationKind::Delete.to_string()),
			])
			.order_by(crdt_operation::timestamp::order(SortOrder::Desc))
			.exec()
			.await?;

		if delete.is_some() {
			debug!("Found a previous delete operation with the same SyncId, will ignore these operations");
			return Ok(());
		}

		handle_crdt_create_and_updates(db, &device_pub_id, model, record_id, ops, timestamp)
			.await?;
	}
	// > 0 Update - batches updates with a fake Create op
	else {
		trace!("Updates operation");

		let mut data = BTreeMap::new();

		for op in ops.into_iter().rev() {
			let CRDTOperationData::Update { field, value } = op.data else {
				unreachable!("Create + Delete should be filtered out!");
			};

			data.insert(field, (value, op.timestamp));
		}

		// conflict resolution
		let (create, updates) = db
			._batch((
				db.crdt_operation()
					.find_first(vec![
						crdt_operation::model::equals(i32::from(model)),
						crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
						crdt_operation::kind::equals(OperationKind::Create.to_string()),
					])
					.order_by(crdt_operation::timestamp::order(SortOrder::Desc)),
				data.iter()
					.map(|(k, (_, timestamp))| {
						Ok(db
							.crdt_operation()
							.find_first(vec![
								crdt_operation::timestamp::gt({
									#[allow(clippy::cast_possible_wrap)]
									// SAFETY: we had to store using i64 due to SQLite limitations
									{
										timestamp.as_u64() as i64
									}
								}),
								crdt_operation::model::equals(i32::from(model)),
								crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
								crdt_operation::kind::equals(OperationKind::Update(k).to_string()),
							])
							.order_by(crdt_operation::timestamp::order(SortOrder::Desc)))
					})
					.collect::<Result<Vec<_>, Error>>()?,
			))
			.await?;

		if create.is_none() {
			warn!("Failed to find a previous create operation with the same SyncId");
			return Ok(());
		}

		handle_crdt_updates(db, &device_pub_id, model, record_id, data, updates).await?;
	}

	// read the timestamp for the operation's device, or insert one if it doesn't exist
	let current_last_timestamp = timestamp_per_device
		.read()
		.await
		.get(&device_pub_id)
		.copied();

	// update the stored timestamp for this device - will be derived from the crdt operations table on restart
	let new_ts = NTP64::max(current_last_timestamp.unwrap_or_default(), new_timestamp);

	timestamp_per_device
		.write()
		.await
		.insert(device_pub_id, new_ts);

	Ok(())
}

async fn handle_crdt_updates(
	db: &PrismaClient,
	device_pub_id: &DevicePubId,
	model_id: ModelId,
	record_id: rmpv::Value,
	mut data: BTreeMap<String, (rmpv::Value, NTP64)>,
	updates: Vec<Option<crdt_operation::Data>>,
) -> Result<(), Error> {
	let keys = data.keys().cloned().collect::<Vec<_>>();
	let device_pub_id = sd_sync::DevicePubId::from(device_pub_id);

	// does the same thing as processing ops one-by-one and returning early if a newer op was found
	for (update, key) in updates.into_iter().zip(keys) {
		if update.is_some() {
			data.remove(&key);
		}
	}

	db._transaction()
		.with_timeout(30 * 10000)
		.with_max_wait(30 * 10000)
		.run(|db| async move {
			// fake operation to batch them all at once
			ModelSyncData::from_op(CRDTOperation {
				device_pub_id,
				model_id,
				record_id: record_id.clone(),
				timestamp: NTP64(0),
				data: CRDTOperationData::Create(
					data.iter()
						.map(|(k, (data, _))| (k.clone(), data.clone()))
						.collect(),
				),
			})
			.ok_or(Error::InvalidModelId(model_id))?
			.exec(&db)
			.await?;

			// need to only apply ops that haven't been filtered out
			data.into_iter()
				.map(|(field, (value, timestamp))| {
					let record_id = record_id.clone();
					let db = &db;

					async move {
						write_crdt_op_to_db(
							&CRDTOperation {
								device_pub_id,
								model_id,
								record_id,
								timestamp,
								data: CRDTOperationData::Update { field, value },
							},
							db,
						)
						.await
					}
				})
				.collect::<Vec<_>>()
				.try_join()
				.await
				.map(|_| ())
		})
		.await
}

async fn handle_crdt_create_and_updates(
	db: &PrismaClient,
	device_pub_id: &DevicePubId,
	model_id: ModelId,
	record_id: rmpv::Value,
	ops: Vec<CompressedCRDTOperation>,
	timestamp: NTP64,
) -> Result<(), Error> {
	let mut data = BTreeMap::new();
	let device_pub_id = sd_sync::DevicePubId::from(device_pub_id);

	let mut applied_ops = vec![];

	// search for all Updates until a Create is found
	for op in ops.into_iter().rev() {
		match &op.data {
			CRDTOperationData::Delete => unreachable!("Delete can't exist here!"),
			CRDTOperationData::Create(create_data) => {
				for (k, v) in create_data {
					data.entry(k.clone()).or_insert_with(|| v.clone());
				}

				applied_ops.push(op);

				break;
			}
			CRDTOperationData::Update { field, value } => {
				data.insert(field.clone(), value.clone());
				applied_ops.push(op);
			}
		}
	}

	db._transaction()
		.with_timeout(30 * 10000)
		.with_max_wait(30 * 10000)
		.run(|db| async move {
			// fake a create with a bunch of data rather than individual insert
			ModelSyncData::from_op(CRDTOperation {
				device_pub_id,
				model_id,
				record_id: record_id.clone(),
				timestamp,
				data: CRDTOperationData::Create(data),
			})
			.ok_or(Error::InvalidModelId(model_id))?
			.exec(&db)
			.await?;

			applied_ops
				.into_iter()
				.map(|CompressedCRDTOperation { timestamp, data }| {
					let record_id = record_id.clone();
					let db = &db;
					async move {
						write_crdt_op_to_db(
							&CRDTOperation {
								device_pub_id,
								timestamp,
								model_id,
								record_id,
								data,
							},
							db,
						)
						.await
					}
				})
				.collect::<Vec<_>>()
				.try_join()
				.await
				.map(|_| ())
		})
		.await
}

async fn handle_crdt_deletion(
	db: &PrismaClient,
	device_pub_id: &DevicePubId,
	model: u16,
	record_id: rmpv::Value,
	delete_op: &CompressedCRDTOperation,
) -> Result<(), Error> {
	// deletes are the be all and end all, no need to check anything
	let op = CRDTOperation {
		device_pub_id: device_pub_id.into(),
		model_id: model,
		record_id,
		timestamp: delete_op.timestamp,
		data: CRDTOperationData::Delete,
	};

	db._transaction()
		.with_timeout(30 * 10000)
		.with_max_wait(30 * 10000)
		.run(|db| async move {
			ModelSyncData::from_op(op.clone())
				.ok_or(Error::InvalidModelId(model))?
				.exec(&db)
				.await?;

			write_crdt_op_to_db(&op, &db).await
		})
		.await
}
