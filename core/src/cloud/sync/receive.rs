use crate::{library::Libraries, Node};

use sd_cloud_api::RequestConfigProvider;
use sd_p2p::RemoteIdentity;
use sd_prisma::prisma::{cloud_crdt_operation, instance, PrismaClient, SortOrder};
use sd_sync::CRDTOperation;
use sd_utils::uuid_to_bytes;

use std::{
	collections::{hash_map::Entry, HashMap},
	str::FromStr,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use base64::prelude::*;
use chrono::Utc;
use serde_json::to_vec;
use tokio::{sync::Notify, time::sleep};
use tracing::{debug, info};
use uuid::Uuid;

use super::{err_break, CompressedCRDTOperations};

// Responsible for downloading sync operations from the cloud to be processed by the ingester

#[allow(clippy::too_many_arguments)]
pub async fn run_actor(
	libraries: Arc<Libraries>,
	db: Arc<PrismaClient>,
	library_id: Uuid,
	instance_uuid: Uuid,
	sync: Arc<sd_core_sync::Manager>,
	ingest_notify: Arc<Notify>,
	node: Arc<Node>,
	active: Arc<AtomicBool>,
	active_notify: Arc<Notify>,
) {
	loop {
		active.store(true, Ordering::Relaxed);
		active_notify.notify_waiters();

		loop {
			// We need to know the latest operations we should be retrieving
			let mut cloud_timestamps = {
				let timestamps = sync.timestamps.read().await;

				// looks up the most recent operation we've received (not ingested!) for each instance
				let db_timestamps = err_break!(
					db._batch(
						timestamps
							.keys()
							.map(|id| {
								db.cloud_crdt_operation()
									.find_first(vec![cloud_crdt_operation::instance::is(vec![
										instance::pub_id::equals(uuid_to_bytes(id)),
									])])
									.order_by(cloud_crdt_operation::timestamp::order(
										SortOrder::Desc,
									))
							})
							.collect::<Vec<_>>()
					)
					.await
				);

				// compares the latest ingested timestamp with the latest received timestamp
				// and picks the highest one for each instance
				let mut cloud_timestamps = db_timestamps
					.into_iter()
					.zip(timestamps.iter())
					.map(|(d, (id, sync_timestamp))| {
						let cloud_timestamp = d.map(|d| d.timestamp).unwrap_or_default() as u64;

						debug!(
							instance_id = %id,
							sync_timestamp = sync_timestamp.as_u64(),
							%cloud_timestamp,
							"Comparing sync timestamps",
						);

						let max_timestamp = Ord::max(cloud_timestamp, sync_timestamp.as_u64());

						(*id, max_timestamp)
					})
					.collect::<HashMap<_, _>>();

				cloud_timestamps.remove(&instance_uuid);

				cloud_timestamps
			};

			let instance_timestamps = sync
				.timestamps
				.read()
				.await
				.keys()
				.map(
					|uuid| sd_cloud_api::library::message_collections::get::InstanceTimestamp {
						instance_uuid: *uuid,
						from_time: cloud_timestamps
							.get(uuid)
							.copied()
							.unwrap_or_default()
							.to_string(),
					},
				)
				.collect();

			let collections = err_break!(
				sd_cloud_api::library::message_collections::get(
					node.get_request_config().await,
					library_id,
					instance_uuid,
					instance_timestamps,
				)
				.await
			);

			info!(
				collections_count = collections.len(),
				"Received collections;",
			);

			if collections.is_empty() {
				break;
			}

			let mut cloud_library_data: Option<Option<sd_cloud_api::Library>> = None;

			for collection in collections {
				if let Entry::Vacant(e) = cloud_timestamps.entry(collection.instance_uuid) {
					let fetched_library = match &cloud_library_data {
						None => {
							let Some(fetched_library) = err_break!(
								sd_cloud_api::library::get(
									node.get_request_config().await,
									library_id
								)
								.await
							) else {
								break;
							};

							cloud_library_data
								.insert(Some(fetched_library))
								.as_ref()
								.expect("error inserting fetched library")
						}
						Some(None) => {
							break;
						}
						Some(Some(fetched_library)) => fetched_library,
					};

					let Some(instance) = fetched_library
						.instances
						.iter()
						.find(|i| i.uuid == collection.instance_uuid)
					else {
						break;
					};

					err_break!(
						upsert_instance(
							library_id,
							&db,
							&sync,
							&libraries,
							&collection.instance_uuid,
							instance.identity,
							&instance.node_id,
							RemoteIdentity::from_str(&instance.node_remote_identity)
								.expect("malformed remote identity in the DB"),
							node.p2p.peer_metadata(),
						)
						.await
					);

					e.insert(0);
				}

				let compressed_operations: CompressedCRDTOperations = err_break!(
					rmp_serde::from_slice(err_break!(&BASE64_STANDARD.decode(collection.contents)))
				);

				let operations = compressed_operations.into_ops();

				debug!(
					instance_id = %collection.instance_uuid,
					start = ?operations.first().map(|operation| operation.timestamp.as_u64()),
					end = ?operations.last().map(|operation| operation.timestamp.as_u64()),
					"Processing collection",
				);

				err_break!(write_cloud_ops_to_db(operations, &db).await);

				let collection_timestamp: u64 =
					collection.end_time.parse().expect("unable to parse time");

				let timestamp = cloud_timestamps
					.entry(collection.instance_uuid)
					.or_insert(collection_timestamp);

				if *timestamp < collection_timestamp {
					*timestamp = collection_timestamp;
				}
			}

			ingest_notify.notify_waiters();
		}

		active.store(false, Ordering::Relaxed);
		active_notify.notify_waiters();

		sleep(Duration::from_secs(60)).await;
	}
}

async fn write_cloud_ops_to_db(
	ops: Vec<CRDTOperation>,
	db: &PrismaClient,
) -> Result<(), prisma_client_rust::QueryError> {
	db._batch(ops.into_iter().map(|op| crdt_op_db(&op).to_query(db)))
		.await?;

	Ok(())
}

fn crdt_op_db(op: &CRDTOperation) -> cloud_crdt_operation::Create {
	cloud_crdt_operation::Create {
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: op.data.as_kind().to_string(),
		data: to_vec(&op.data).expect("unable to serialize data"),
		model: op.model as i32,
		record_id: rmp_serde::to_vec(&op.record_id).expect("unable to serialize record id"),
		_params: vec![],
	}
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_instance(
	library_id: Uuid,
	db: &PrismaClient,
	sync: &sd_core_sync::Manager,
	libraries: &Libraries,
	uuid: &Uuid,
	identity: RemoteIdentity,
	node_id: &Uuid,
	node_remote_identity: RemoteIdentity,
	metadata: HashMap<String, String>,
) -> prisma_client_rust::Result<()> {
	db.instance()
		.upsert(
			instance::pub_id::equals(uuid_to_bytes(uuid)),
			instance::create(
				uuid_to_bytes(uuid),
				identity.get_bytes().to_vec(),
				node_id.as_bytes().to_vec(),
				Utc::now().into(),
				Utc::now().into(),
				vec![
					instance::node_remote_identity::set(Some(
						node_remote_identity.get_bytes().to_vec(),
					)),
					instance::metadata::set(Some(
						serde_json::to_vec(&metadata).expect("unable to serialize metadata"),
					)),
				],
			),
			vec![],
		)
		.exec()
		.await?;

	sync.timestamps.write().await.entry(*uuid).or_default();

	// Called again so the new instances are picked up
	libraries.update_instances_by_id(library_id).await;

	Ok(())
}
