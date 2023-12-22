use crate::{
	cloud::sync::{err_break, err_return},
	library::Library,
	Node,
};

use sd_core_sync::NTP64;
use sd_prisma::prisma::{cloud_crdt_operation, instance, PrismaClient, SortOrder};
use sd_sync::*;
use sd_utils::{from_bytes_to_uuid, uuid_to_bytes};

use std::{
	collections::{hash_map::Entry, HashMap},
	sync::Arc,
	time::Duration,
};

use base64::prelude::*;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, to_vec};
use tokio::{sync::Notify, time::sleep};
use uuid::Uuid;

pub async fn run_actor((library, node, ingest_notify): (Arc<Library>, Arc<Node>, Arc<Notify>)) {
	let db = &library.db;
	let api_url = &library.env.api_url;
	let library_id = library.id;

	let mut cloud_timestamps = {
		let timestamps = library.sync.timestamps.read().await;

		let batch = timestamps
			.keys()
			.map(|id| {
				db.cloud_crdt_operation()
					.find_first(vec![cloud_crdt_operation::instance::is(vec![
						instance::pub_id::equals(uuid_to_bytes(*id)),
					])])
					.order_by(cloud_crdt_operation::timestamp::order(SortOrder::Desc))
			})
			.collect::<Vec<_>>();

		err_return!(db._batch(batch).await)
			.into_iter()
			.zip(timestamps.keys())
			.map(|(d, id)| {
				let cloud_timestamp = NTP64(d.map(|d| d.timestamp).unwrap_or_default() as u64);
				let sync_timestamp = *timestamps
					.get(id)
					.expect("unable to find matching timestamp");

				let max_timestamp = Ord::max(cloud_timestamp, sync_timestamp);

				(*id, max_timestamp)
			})
			.collect::<HashMap<_, _>>()
	};

	loop {
		let instances = {
			err_break!(
				db.instance()
					.find_many(vec![])
					.select(instance::select!({ pub_id }))
					.exec()
					.await
			)
			.into_iter()
			.map(|i| {
				let uuid = from_bytes_to_uuid(&i.pub_id);

				json!({
					"instanceUuid": uuid,
					"fromTime": cloud_timestamps.get(&uuid).cloned().unwrap_or_default().as_u64().to_string()
				})
			})
			.collect::<Vec<_>>()
		};

		#[derive(Deserialize, Debug)]
		#[serde(rename_all = "camelCase")]
		struct MessageCollection {
			instance_uuid: Uuid,
			// start_time: String,
			end_time: String,
			contents: String,
		}

		{
			let collections = node
				.authed_api_request(
					node.http
						.post(&format!(
							"{api_url}/api/v1/libraries/{library_id}/messageCollections/get"
						))
						.json(&json!({
							"instanceUuid": library.instance_uuid,
							"timestamps": instances
						})),
				)
				.await
				.expect("couldn't get response")
				.json::<Vec<MessageCollection>>()
				.await
				.expect("couldn't deserialize response");

			let mut cloud_library_data: Option<Option<sd_cloud_api::Library>> = None;

			for collection in collections {
				if let Entry::Vacant(e) = cloud_timestamps.entry(collection.instance_uuid) {
					let fetched_library = match &cloud_library_data {
						None => {
							let Some(fetched_library) = err_break!(
								sd_cloud_api::library::get(
									node.cloud_api_config().await,
									library.id
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
							db,
							collection.instance_uuid,
							err_break!(BASE64_STANDARD.decode(instance.identity.clone()))
						)
						.await
					);

					e.insert(NTP64(0));
				}

				err_break!(
					write_cloud_ops_to_db(
						err_break!(serde_json::from_slice(err_break!(
							&BASE64_STANDARD.decode(collection.contents)
						))),
						db
					)
					.await
				);

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
		kind: op.kind().to_string(),
		data: to_vec(&op.data).expect("unable to serialize data"),
		model: op.model.to_string(),
		record_id: to_vec(&op.record_id).expect("unable to serialize record id"),
		_params: vec![],
	}
}

async fn create_instance(
	db: &PrismaClient,
	uuid: Uuid,
	identity: Vec<u8>,
) -> prisma_client_rust::Result<()> {
	db.instance()
		.upsert(
			instance::pub_id::equals(uuid_to_bytes(uuid)),
			instance::create(
				uuid_to_bytes(uuid),
				identity,
				vec![],
				"".to_string(),
				0,
				Utc::now().into(),
				Utc::now().into(),
				vec![],
			),
			vec![],
		)
		.exec()
		.await?;

	Ok(())
}
