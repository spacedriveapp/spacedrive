use crate::CloudServices;

use sd_cloud_schema::sync::groups;
use sd_core_sync::{cloud_crdt_op_db, CRDTOperation, DevicePubId, SyncManager};

use sd_actors::Stopper;
use sd_prisma::prisma::{cloud_crdt_operation, device, instance, PrismaClient};
use sd_utils::uuid_to_bytes;

use std::{
	collections::HashMap,
	sync::{atomic::AtomicBool, Arc},
};

use chrono::Utc;
use futures::FutureExt;
use serde_json::to_vec;
use tokio::sync::Notify;
use uuid::Uuid;

// Responsible for downloading sync operations from the cloud to be processed by the ingester

pub async fn run_actor(
	db: Arc<PrismaClient>,
	sync_group_pub_id: groups::PubId,
	cloud_services: Arc<CloudServices>,
	sync: SyncManager,
	ingest_notify: Arc<Notify>,
	(active, active_notify): (Arc<AtomicBool>, Arc<Notify>),
	stop: Stopper,
) {
	// enum Race {
	// 	Continue,
	// 	Stop,
	// }

	// loop {
	// 	active.store(true, Ordering::Relaxed);
	// 	active_notify.notify_waiters();

	// 	loop {
	// 		// We need to know the latest operations we should be retrieving
	// 		let mut cloud_timestamps = {
	// 			let timestamps = sync.timestamps.read().await;

	// 			// looks up the most recent operation we've received (not ingested!) for each instance
	// 			let db_timestamps = err_break!(
	// 				db._batch(
	// 					timestamps
	// 						.keys()
	// 						.map(|id| {
	// 							db.cloud_crdt_operation()
	// 								.find_first(vec![cloud_crdt_operation::instance::is(vec![
	// 									instance::pub_id::equals(uuid_to_bytes(id)),
	// 								])])
	// 								.order_by(cloud_crdt_operation::timestamp::order(
	// 									SortOrder::Desc,
	// 								))
	// 						})
	// 						.collect::<Vec<_>>()
	// 				)
	// 				.await
	// 			);

	// 			// compares the latest ingested timestamp with the latest received timestamp
	// 			// and picks the highest one for each instance
	// 			let mut cloud_timestamps = db_timestamps
	// 				.into_iter()
	// 				.zip(timestamps.iter())
	// 				.map(|(d, (id, sync_timestamp))| {
	// 					let cloud_timestamp = d.map(|d| d.timestamp).unwrap_or_default() as u64;

	// 					debug!(
	// 						instance_id = %id,
	// 						sync_timestamp = sync_timestamp.as_u64(),
	// 						%cloud_timestamp,
	// 						"Comparing sync timestamps",
	// 					);

	// 					let max_timestamp = Ord::max(cloud_timestamp, sync_timestamp.as_u64());

	// 					(*id, max_timestamp)
	// 				})
	// 				.collect::<HashMap<_, _>>();

	// 			cloud_timestamps.remove(&instance_uuid);

	// 			cloud_timestamps
	// 		};

	// 		let instance_timestamps: Vec<InstanceTimestamp> = sync
	// 			.timestamps
	// 			.read()
	// 			.await
	// 			.keys()
	// 			.map(
	// 				|uuid| sd_cloud_api::library::message_collections::get::InstanceTimestamp {
	// 					instance_uuid: *uuid,
	// 					from_time: cloud_timestamps
	// 						.get(uuid)
	// 						.copied()
	// 						.unwrap_or_default()
	// 						.to_string(),
	// 				},
	// 			)
	// 			.collect();

	// 		let collections = err_break!(
	// 			sd_cloud_api::library::message_collections::get(
	// 				node.get_request_config().await,
	// 				library_id,
	// 				instance_uuid,
	// 				instance_timestamps,
	// 			)
	// 			.await
	// 		);

	// 		info!(
	// 			collections_count = collections.len(),
	// 			"Received collections;",
	// 		);

	// 		if collections.is_empty() {
	// 			break;
	// 		}

	// 		let mut cloud_library_data: Option<Option<sd_cloud_api::Library>> = None;

	// 		for collection in collections {
	// 			if let Entry::Vacant(e) = cloud_timestamps.entry(collection.instance_uuid) {
	// 				let fetched_library = match &cloud_library_data {
	// 					None => {
	// 						let Some(fetched_library) = err_break!(
	// 							sd_cloud_api::library::get(
	// 								node.get_request_config().await,
	// 								library_id
	// 							)
	// 							.await
	// 						) else {
	// 							break;
	// 						};

	// 						cloud_library_data
	// 							.insert(Some(fetched_library))
	// 							.as_ref()
	// 							.expect("error inserting fetched library")
	// 					}
	// 					Some(None) => {
	// 						break;
	// 					}
	// 					Some(Some(fetched_library)) => fetched_library,
	// 				};

	// 				let Some(instance) = fetched_library
	// 					.instances
	// 					.iter()
	// 					.find(|i| i.uuid == collection.instance_uuid)
	// 				else {
	// 					break;
	// 				};

	// 				err_break!(
	// 					upsert_instance(
	// 						library_id,
	// 						&db,
	// 						&sync,
	// 						&libraries,
	// 						&collection.instance_uuid,
	// 						instance.identity,
	// 						&instance.node_id,
	// 						RemoteIdentity::from_str(&instance.node_remote_identity)
	// 							.expect("malformed remote identity in the DB"),
	// 						node.p2p.peer_metadata(),
	// 					)
	// 					.await
	// 				);

	// 				e.insert(0);
	// 			}

	// 			let compressed_operations: CompressedCRDTOperations = err_break!(
	// 				rmp_serde::from_slice(err_break!(&BASE64_STANDARD.decode(collection.contents)))
	// 			);

	// 			let operations = compressed_operations.into_ops();

	// 			debug!(
	// 				instance_id = %collection.instance_uuid,
	// 				start = ?operations.first().map(|operation| operation.timestamp.as_u64()),
	// 				end = ?operations.last().map(|operation| operation.timestamp.as_u64()),
	// 				"Processing collection",
	// 			);

	// 			err_break!(write_cloud_ops_to_db(operations, &db).await);

	// 			let collection_timestamp: u64 =
	// 				collection.end_time.parse().expect("unable to parse time");

	// 			let timestamp = cloud_timestamps
	// 				.entry(collection.instance_uuid)
	// 				.or_insert(collection_timestamp);

	// 			if *timestamp < collection_timestamp {
	// 				*timestamp = collection_timestamp;
	// 			}
	// 		}

	// 		ingest_notify.notify_waiters();
	// 	}

	// 	active.store(false, Ordering::Relaxed);
	// 	active_notify.notify_waiters();

	// 	if let Race::Stop = (
	// 		sleep(Duration::from_secs(60)).map(|()| Race::Continue),
	// 		stop.into_future().map(|()| Race::Stop),
	// 	)
	// 		.race()
	// 		.await
	// 	{
	// 		break;
	// 	}
	// }
}

pub async fn write_cloud_ops_to_db(
	ops: Vec<CRDTOperation>,
	db: &PrismaClient,
) -> Result<(), sd_core_sync::Error> {
	db._batch(
		ops.into_iter()
			.map(|op| cloud_crdt_op_db(&op).map(|op| op.to_query(db)))
			.collect::<Result<Vec<_>, _>>()?,
	)
	.await?;

	Ok(())
}
