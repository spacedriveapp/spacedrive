use crate::library::{Libraries, Library};

use super::{err_break, CompressedCRDTOperations};
use sd_cloud_api::RequestConfigProvider;
use sd_core_sync::NTP64;
use sd_p2p2::{IdentityOrRemoteIdentity, RemoteIdentity};
use sd_prisma::prisma::{cloud_crdt_operation, instance, PrismaClient, SortOrder};
use sd_sync::CRDTOperation;
use sd_utils::uuid_to_bytes;
use tracing::info;

use std::{
	collections::{hash_map::Entry, HashMap},
	sync::Arc,
	time::Duration,
};

use base64::prelude::*;
use chrono::Utc;
use serde_json::to_vec;
use tokio::{sync::Notify, time::sleep};
use uuid::Uuid;

//// Responsible for downloading sync operations from the cloud to be processed by the ingester

pub async fn run_actor(
	library: Arc<Library>,
	libraries: Arc<Libraries>,
	db: Arc<PrismaClient>,
	library_id: Uuid,
	instance_uuid: Uuid,
	sync: Arc<sd_core_sync::Manager>,
	cloud_api_config_provider: Arc<impl RequestConfigProvider>,
	ingest_notify: Arc<Notify>,
) {
	loop {
		loop {
			let mut cloud_timestamps = {
				let timestamps = sync.timestamps.read().await;

				err_break!(
					db._batch(
						timestamps
							.keys()
							.map(|id| {
								db.cloud_crdt_operation()
									.find_first(vec![cloud_crdt_operation::instance::is(vec![
										instance::pub_id::equals(uuid_to_bytes(*id)),
									])])
									.order_by(cloud_crdt_operation::timestamp::order(
										SortOrder::Desc,
									))
							})
							.collect::<Vec<_>>()
					)
					.await
				)
				.into_iter()
				.zip(timestamps.iter())
				.map(|(d, (id, sync_timestamp))| {
					let cloud_timestamp = NTP64(d.map(|d| d.timestamp).unwrap_or_default() as u64);

					let max_timestamp = Ord::max(cloud_timestamp, *sync_timestamp);

					(*id, max_timestamp)
				})
				.collect::<HashMap<_, _>>()
			};

			info!(
				"Fetched timestamps for {} local instances",
				cloud_timestamps.len()
			);

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
							.cloned()
							.unwrap_or_default()
							.as_u64()
							.to_string(),
					},
				)
				.collect();

			let collections = err_break!(
				sd_cloud_api::library::message_collections::get(
					cloud_api_config_provider.get_request_config().await,
					library_id,
					instance_uuid,
					instance_timestamps,
				)
				.await
			);

			info!("Received {} collections", collections.len());

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
									cloud_api_config_provider.get_request_config().await,
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
						create_instance(
							&library,
							&libraries,
							collection.instance_uuid,
							instance.identity,
							instance.node_id,
							instance.node_name.clone(),
							instance.node_platform,
						)
						.await
					);

					e.insert(NTP64(0));
				}

				let compressed_operations: CompressedCRDTOperations = err_break!(
					rmp_serde::from_slice(err_break!(&BASE64_STANDARD.decode(collection.contents)))
				);

				err_break!(write_cloud_ops_to_db(compressed_operations.into_ops(), &db).await);

				let collection_timestamp =
					NTP64(collection.end_time.parse().expect("unable to parse time"));

				let timestamp = cloud_timestamps
					.entry(collection.instance_uuid)
					.or_insert(collection_timestamp);

				if *timestamp < collection_timestamp {
					*timestamp = collection_timestamp;
				}
			}

			ingest_notify.notify_waiters();
		}

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
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: op.data.as_kind().to_string(),
		data: to_vec(&op.data).expect("unable to serialize data"),
		model: op.model.to_string(),
		record_id: to_vec(&op.record_id).expect("unable to serialize record id"),
		_params: vec![],
	}
}

pub async fn create_instance(
	library: &Arc<Library>,
	libraries: &Libraries,
	uuid: Uuid,
	identity: RemoteIdentity,
	node_id: Uuid,
	node_name: String,
	node_platform: u8,
) -> prisma_client_rust::Result<()> {
	library
		.db
		.instance()
		.upsert(
			instance::pub_id::equals(uuid_to_bytes(uuid)),
			instance::create(
				uuid_to_bytes(uuid),
				IdentityOrRemoteIdentity::RemoteIdentity(identity).to_bytes(),
				node_id.as_bytes().to_vec(),
				node_name,
				node_platform as i32,
				Utc::now().into(),
				Utc::now().into(),
				vec![],
			),
			vec![],
		)
		.exec()
		.await?;

	library.sync.timestamps.write().await.insert(uuid, NTP64(0));

	// Called again so the new instances are picked up
	libraries.update_instances(library.clone()).await;

	Ok(())
}
