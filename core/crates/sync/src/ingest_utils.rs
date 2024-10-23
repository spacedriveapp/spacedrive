use sd_core_prisma_helpers::DevicePubId;

use sd_prisma::{
	prisma::{crdt_operation, PrismaClient},
	prisma_sync::ModelSyncData,
};
use sd_sync::{
	CRDTOperation, CRDTOperationData, CompressedCRDTOperation, ModelId, OperationKind, RecordId,
};

use std::{collections::BTreeMap, num::NonZeroU128, sync::Arc};

use futures_concurrency::future::TryJoin;
use tokio::sync::Mutex;
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
	sync_lock: Arc<Mutex<()>>,
	db: &PrismaClient,
	device_pub_id: DevicePubId,
	model_id: ModelId,
	(record_id, mut ops): (RecordId, Vec<CompressedCRDTOperation>),
) -> Result<(), Error> {
	ops.sort_by_key(|op| op.timestamp);

	let new_timestamp = ops.last().expect("Empty ops array").timestamp;

	update_clock(clock, new_timestamp, &device_pub_id);

	// Delete - ignores all other messages
	if let Some(delete_op) = ops
		.iter()
		.rev()
		.find(|op| matches!(op.data, CRDTOperationData::Delete))
	{
		trace!("Deleting operation");
		handle_crdt_deletion(
			db,
			&sync_lock,
			&device_pub_id,
			model_id,
			record_id,
			delete_op,
		)
		.await?;
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

		handle_crdt_create_and_updates(
			db,
			&sync_lock,
			&device_pub_id,
			model_id,
			record_id,
			ops,
			timestamp,
		)
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

		let earlier_time = data.values().fold(
			NTP64(u64::from(u32::MAX)),
			|earlier_time, (_, timestamp)| {
				if timestamp.0 < earlier_time.0 {
					*timestamp
				} else {
					earlier_time
				}
			},
		);

		// conflict resolution
		let (create, possible_newer_updates_count) = db
			._batch((
				db.crdt_operation().count(vec![
					crdt_operation::model::equals(i32::from(model_id)),
					crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
					crdt_operation::kind::equals(OperationKind::Create.to_string()),
				]),
				// Fetching all update operations newer than our current earlier timestamp
				db.crdt_operation()
					.find_many(vec![
						crdt_operation::timestamp::gt({
							#[allow(clippy::cast_possible_wrap)]
							// SAFETY: we had to store using i64 due to SQLite limitations
							{
								earlier_time.as_u64() as i64
							}
						}),
						crdt_operation::model::equals(i32::from(model_id)),
						crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
						crdt_operation::kind::starts_with("u".to_string()),
					])
					.select(crdt_operation::select!({ kind timestamp })),
			))
			.await?;

		if create == 0 {
			warn!("Failed to find a previous create operation with the same SyncId");
			return Ok(());
		}

		for candidate in possible_newer_updates_count {
			// The first element is "u" meaning that this is an update, so we skip it
			for key in candidate
				.kind
				.split(':')
				.filter(|field| !field.is_empty())
				.skip(1)
			{
				// remove entries if we possess locally more recent updates for this field
				if data.get(key).is_some_and(|(_, new_timestamp)| {
					#[allow(clippy::cast_sign_loss)]
					{
						// we need to store as i64 due to SQLite limitations
						*new_timestamp < NTP64(candidate.timestamp as u64)
					}
				}) {
					data.remove(key);
				}
			}

			if data.is_empty() {
				break;
			}
		}

		handle_crdt_updates(db, &sync_lock, &device_pub_id, model_id, record_id, data).await?;
	}

	update_timestamp_per_device(timestamp_per_device, device_pub_id, new_timestamp).await;

	Ok(())
}

pub async fn bulk_ingest_create_only_ops(
	clock: &HLC,
	timestamp_per_device: &TimestampPerDevice,
	db: &PrismaClient,
	device_pub_id: DevicePubId,
	model_id: ModelId,
	ops: Vec<(RecordId, CompressedCRDTOperation)>,
	sync_lock: Arc<Mutex<()>>,
) -> Result<(), Error> {
	let latest_timestamp = ops.iter().fold(NTP64(0), |latest, (_, op)| {
		if latest < op.timestamp {
			op.timestamp
		} else {
			latest
		}
	});

	update_clock(clock, latest_timestamp, &device_pub_id);

	let ops = ops
		.into_iter()
		.map(|(record_id, op)| {
			rmp_serde::to_vec(&record_id)
				.map(|serialized_record_id| (record_id, serialized_record_id, op))
		})
		.collect::<Result<Vec<_>, _>>()?;

	// conflict resolution
	let delete_counts = db
		._batch(
			ops.iter()
				.map(|(_, serialized_record_id, _)| {
					db.crdt_operation().count(vec![
						crdt_operation::model::equals(i32::from(model_id)),
						crdt_operation::record_id::equals(serialized_record_id.clone()),
						crdt_operation::kind::equals(OperationKind::Delete.to_string()),
					])
				})
				.collect::<Vec<_>>(),
		)
		.await?;

	let lock_guard = sync_lock.lock().await;

	db._transaction()
		.with_timeout(30 * 10000)
		.with_max_wait(30 * 10000)
		.run(|db| {
			let device_pub_id = device_pub_id.clone();

			async move {
				// complying with borrowck
				let device_pub_id = &device_pub_id;

				let (crdt_creates, model_sync_data) = ops
					.into_iter()
					.zip(delete_counts)
					.filter_map(|(data, delete_count)| (delete_count == 0).then_some(data))
					.map(
						|(
							record_id,
							serialized_record_id,
							CompressedCRDTOperation { timestamp, data },
						)| {
							let crdt_create = crdt_operation::CreateUnchecked {
								timestamp: {
									#[allow(clippy::cast_possible_wrap)]
									// SAFETY: we have to store using i64 due to SQLite limitations
									{
										timestamp.0 as i64
									}
								},
								model: i32::from(model_id),
								record_id: serialized_record_id,
								kind: "c".to_string(),
								data: rmp_serde::to_vec(&data)?,
								device_pub_id: device_pub_id.to_db(),
								_params: vec![],
							};

							// NOTE(@fogodev): I wish I could do a create many here instead of creating separately each
							// entry, but it's not supported by PCR
							let model_sync_data = ModelSyncData::from_op(CRDTOperation {
								device_pub_id: Uuid::from(device_pub_id),
								model_id,
								record_id,
								timestamp,
								data,
							})?
							.exec(&db);

							Ok::<_, Error>((crdt_create, model_sync_data))
						},
					)
					.collect::<Result<Vec<_>, _>>()?
					.into_iter()
					.unzip::<_, _, Vec<_>, Vec<_>>();

				model_sync_data.try_join().await?;

				db.crdt_operation().create_many(crdt_creates).exec().await?;

				Ok::<_, Error>(())
			}
		})
		.await?;

	drop(lock_guard);

	update_timestamp_per_device(timestamp_per_device, device_pub_id, latest_timestamp).await;

	Ok(())
}

#[instrument(skip_all, err)]
async fn handle_crdt_updates(
	db: &PrismaClient,
	sync_lock: &Mutex<()>,
	device_pub_id: &DevicePubId,
	model_id: ModelId,
	record_id: rmpv::Value,
	data: BTreeMap<String, (rmpv::Value, NTP64)>,
) -> Result<(), Error> {
	let device_pub_id = sd_sync::DevicePubId::from(device_pub_id);

	let _lock_guard = sync_lock.lock().await;

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

#[instrument(skip_all, err)]
async fn handle_crdt_create_and_updates(
	db: &PrismaClient,
	sync_lock: &Mutex<()>,
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

	let _lock_guard = sync_lock.lock().await;

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

#[instrument(skip_all, err)]
async fn handle_crdt_deletion(
	db: &PrismaClient,
	sync_lock: &Mutex<()>,
	device_pub_id: &DevicePubId,
	model: u16,
	record_id: rmpv::Value,
	delete_op: &CompressedCRDTOperation,
) -> Result<(), Error> {
	// deletes are the be all and end all, except if we never created the object to begin with
	// in this case we don't need to delete anything

	if db
		.crdt_operation()
		.count(vec![
			crdt_operation::model::equals(i32::from(model)),
			crdt_operation::record_id::equals(rmp_serde::to_vec(&record_id)?),
		])
		.exec()
		.await?
		== 0
	{
		// This means that in the other device this entry was created and deleted, before this
		// device here could even take notice of it. So we don't need to do anything here.
		return Ok(());
	}

	let op = CRDTOperation {
		device_pub_id: device_pub_id.into(),
		model_id: model,
		record_id,
		timestamp: delete_op.timestamp,
		data: CRDTOperationData::Delete,
	};

	let _lock_guard = sync_lock.lock().await;

	db._transaction()
		.with_timeout(30 * 10000)
		.with_max_wait(30 * 10000)
		.run(|db| async move {
			ModelSyncData::from_op(op.clone())?.exec(&db).await?;

			write_crdt_op_to_db(&op, &db).await
		})
		.await
}

fn update_clock(clock: &HLC, latest_timestamp: NTP64, device_pub_id: &DevicePubId) {
	// first, we update the HLC's timestamp with the incoming one.
	// this involves a drift check + sets the last time of the clock
	clock
		.update_with_timestamp(&Timestamp::new(
			latest_timestamp,
			uhlc::ID::from(
				NonZeroU128::new(Uuid::from(device_pub_id).to_u128_le()).expect("Non zero id"),
			),
		))
		.expect("timestamp has too much drift!");
}

async fn update_timestamp_per_device(
	timestamp_per_device: &TimestampPerDevice,
	device_pub_id: DevicePubId,
	latest_timestamp: NTP64,
) {
	// read the timestamp for the operation's device, or insert one if it doesn't exist
	let current_last_timestamp = timestamp_per_device
		.read()
		.await
		.get(&device_pub_id)
		.copied();

	// update the stored timestamp for this device - will be derived from the crdt operations table on restart
	let new_ts = NTP64::max(current_last_timestamp.unwrap_or_default(), latest_timestamp);

	timestamp_per_device
		.write()
		.await
		.insert(device_pub_id, new_ts);
}
