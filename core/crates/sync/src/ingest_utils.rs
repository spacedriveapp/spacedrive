use sd_core_prisma_helpers::DevicePubId;

use sd_prisma::{
	prisma::{crdt_operation, PrismaClient},
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

crdt_operation::select!(crdt_operation_id { id });

// where the magic happens
#[instrument(skip(clock, ops), fields(operations_count = %ops.len()), err)]
pub async fn process_crdt_operations(
	clock: &HLC,
	timestamp_per_device: &TimestampPerDevice,
	db: &PrismaClient,
	device_pub_id: DevicePubId,
	model_id: ModelId,
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
		handle_crdt_deletion(db, &device_pub_id, model_id, record_id, delete_op).await?;
	}
	// Create + > 0 Update - overwrites the create's data with the updates
	else if let Some(timestamp) = ops
		.iter()
		.rev()
		.find_map(|op| matches!(&op.data, CRDTOperationData::Create(_)).then_some(op.timestamp))
	{
		trace!("Create + Updates operations");

		// conflict resolution
		let delete_count = db
			.crdt_operation()
			.count(vec![
				crdt_operation::model::equals(i32::from(model_id)),
				crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
				crdt_operation::kind::equals(OperationKind::Delete.to_string()),
			])
			.exec()
			.await?;

		if delete_count > 0 {
			debug!("Found a previous delete operation with the same SyncId, will ignore these operations");
			return Ok(());
		}

		handle_crdt_create_and_updates(db, &device_pub_id, model_id, record_id, ops, timestamp)
			.await?;
	}
	// > 0 Update - batches updates with a fake Create op
	else {
		trace!("Updates operation");

		let mut data = BTreeMap::new();

		for op in ops.into_iter().rev() {
			let CRDTOperationData::Update(fields_and_values) = op.data else {
				unreachable!("Create + Delete should be filtered out!");
			};

			for (field, value) in fields_and_values {
				data.insert(field, (value, op.timestamp));
			}
		}

		// conflict resolution
		let (create, newer_updates_count) = db
			._batch((
				db.crdt_operation().count(vec![
					crdt_operation::model::equals(i32::from(model_id)),
					crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
					crdt_operation::kind::equals(OperationKind::Create.to_string()),
				]),
				data.iter()
					.map(|(k, (_, timestamp))| {
						Ok(db.crdt_operation().count(vec![
							crdt_operation::timestamp::gt({
								#[allow(clippy::cast_possible_wrap)]
								// SAFETY: we had to store using i64 due to SQLite limitations
								{
									timestamp.as_u64() as i64
								}
							}),
							crdt_operation::model::equals(i32::from(model_id)),
							crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
							crdt_operation::kind::contains(format!(":{k}:")),
						]))
					})
					.collect::<Result<Vec<_>, Error>>()?,
			))
			.await?;

		if create == 0 {
			warn!("Failed to find a previous create operation with the same SyncId");
			return Ok(());
		}

		let keys = data.keys().cloned().collect::<Vec<_>>();

		// remove entries if we possess locally more recent updates for this field
		for (update, key) in newer_updates_count.into_iter().zip(keys) {
			if update > 0 {
				data.remove(&key);
			}
		}

		handle_crdt_updates(db, &device_pub_id, model_id, record_id, data).await?;
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
	data: BTreeMap<String, (rmpv::Value, NTP64)>,
) -> Result<(), Error> {
	let device_pub_id = sd_sync::DevicePubId::from(device_pub_id);

	db._transaction()
		.with_timeout(30 * 10000)
		.with_max_wait(30 * 10000)
		.run(|db| async move {
			// fake operation to batch them all at once, inserting the latest data on appropriate table
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
			})?
			.exec(&db)
			.await?;

			let (fields_and_values, latest_timestamp) = data.into_iter().fold(
				(BTreeMap::new(), NTP64::default()),
				|(mut fields_and_values, mut latest_time_stamp), (field, (value, timestamp))| {
					fields_and_values.insert(field, value);
					if timestamp > latest_time_stamp {
						latest_time_stamp = timestamp;
					}
					(fields_and_values, latest_time_stamp)
				},
			);

			write_crdt_op_to_db(
				&CRDTOperation {
					device_pub_id,
					model_id,
					record_id,
					timestamp: latest_timestamp,
					data: CRDTOperationData::Update(fields_and_values),
				},
				&db,
			)
			.await
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
			CRDTOperationData::Update(fields_and_values) => {
				for (field, value) in fields_and_values {
					data.insert(field.clone(), value.clone());
				}

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
			})?
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
			ModelSyncData::from_op(op.clone())?.exec(&db).await?;

			write_crdt_op_to_db(&op, &db).await
		})
		.await
}
