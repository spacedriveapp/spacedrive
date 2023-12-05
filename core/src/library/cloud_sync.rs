use super::Library;
use crate::Node;
use base64::prelude::*;
use chrono::Utc;
use itertools::{Either, Itertools};
use sd_core_sync::{GetOpsArgs, SyncMessage, NTP64};
use sd_prisma::prisma::{
	cloud_relation_operation, cloud_shared_operation, instance, relation_operation,
	shared_operation, PrismaClient, SortOrder,
};
use sd_sync::*;
use sd_utils::{from_bytes_to_uuid, uuid_to_bytes};
use serde::Deserialize;
use serde_json::{json, to_vec};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::Notify, time::sleep};
use uuid::Uuid;

macro_rules! err_break {
	($e:expr) => {
		match $e {
			Ok(d) => d,
			Err(e) => {
				tracing::error!("{e}");
				break;
			}
		}
	};
}
macro_rules! return_break {
	($e:expr) => {
		match $e {
			Ok(d) => d,
			Err(e) => {
				tracing::error!("{e}");
				return;
			}
		}
	};
}

pub fn spawn_actors(library: &Arc<Library>, node: &Arc<Node>) {
	let ingest_notify = Arc::new(Notify::new());

	tokio::spawn(send_actor(library.clone(), node.clone()));
	tokio::spawn(receive_actor(
		library.clone(),
		node.clone(),
		ingest_notify.clone(),
	));
	tokio::spawn(ingest_actor(library.clone(), ingest_notify));
}

async fn send_actor(library: Arc<Library>, node: Arc<Node>) {
	let db = &library.db;
	let api_url = &library.env.api_url;
	let library_id = library.id;

	loop {
		println!("send_actor run");

		{
			// recreate subscription each time so that existing messages are dropped
			let mut rx = library.sync.subscribe();

			// wait until Created message comes in
			loop {
				if let Ok(SyncMessage::Created) = rx.recv().await {
					break;
				};
			}
		}

		println!("send_actor sleeping");

		sleep(Duration::from_millis(1000)).await;

		println!("send_actor sending");

		loop {
			let instances = err_break!(
				db.instance()
					.find_many(vec![])
					.select(instance::select!({ pub_id }))
					.exec()
					.await
			)
			.into_iter()
			.map(|i| json!({ "instanceUuid": library.instance_uuid.to_string() }))
			.collect::<Vec<_>>();

			#[derive(Deserialize, Debug)]
			#[serde(rename_all = "camelCase")]
			struct RequestAdd {
				instance_uuid: Uuid,
				from_time: Option<String>,
				// mutex key on the instance
				key: String,
			}

			let req_adds = err_break!(
				err_break!(
					node.authed_api_request(
						node.http
							.post(&format!(
							"{api_url}/api/v1/libraries/{library_id}/messageCollections/requestAdd"
						))
							.json(&json!({ "instances": instances })),
					)
					.await
				)
				.json::<Vec<RequestAdd>>()
				.await
			);

			println!("Add Requests: {req_adds:#?}");

			let mut instances = vec![];

			for req_add in req_adds {
				let ops = err_break!(
					library
						.sync
						.get_ops(GetOpsArgs {
							count: 50,
							clocks: vec![(
								req_add.instance_uuid,
								NTP64(
									req_add
										.from_time
										.unwrap_or_else(|| "0".to_string())
										.parse()
										.unwrap(),
								),
							)],
						})
						.await
				);

				if ops.len() == 0 {
					continue;
				}

				let start_time = ops[0].timestamp.0.to_string();
				let end_time = ops[ops.len() - 1].timestamp.0.to_string();

				instances.push(json!({
					"uuid": req_add.instance_uuid,
					"key": req_add.key,
					"startTime": start_time,
					"endTime": end_time,
					"contents": ops,
				}))
			}

			tracing::debug!("Number of instances: {}", instances.len());
			tracing::debug!(
				"Number of messages: {}",
				instances
					.iter()
					.map(|i| i["contents"].as_array().unwrap().len())
					.sum::<usize>()
			);

			if instances.len() == 0 {
				break;
			}

			#[derive(Deserialize, Debug)]
			#[serde(rename_all = "camelCase")]
			struct DoAdd {
				instance_uuid: Uuid,
				from_time: String,
			}

			let responses = err_break!(
				err_break!(
					node.authed_api_request(
						node.http
							.post(&format!(
								"{api_url}/api/v1/libraries/{library_id}/messageCollections/doAdd",
							))
							.json(&json!({ "instances": instances })),
					)
					.await
				)
				.json::<Vec<DoAdd>>()
				.await
			);

			println!("DoAdd Responses: {responses:#?}");
		}
	}
}

async fn get_latest_timestamp(db: &PrismaClient, instance: Uuid) -> i64 {
	let shared = db
		.shared_operation()
		.find_first(vec![shared_operation::instance::is(vec![
			instance::pub_id::equals(uuid_to_bytes(instance)),
		])])
		.order_by(shared_operation::timestamp::order(SortOrder::Desc))
		.select(shared_operation::select!({ timestamp }))
		.exec()
		.await
		.unwrap()
		.map(|d| d.timestamp)
		.unwrap_or_default();

	let relation = db
		.relation_operation()
		.find_first(vec![relation_operation::instance::is(vec![
			instance::pub_id::equals(uuid_to_bytes(instance)),
		])])
		.order_by(relation_operation::timestamp::order(SortOrder::Desc))
		.select(relation_operation::select!({ timestamp }))
		.exec()
		.await
		.unwrap()
		.map(|d| d.timestamp)
		.unwrap_or_default();

	shared.max(relation)
}

async fn receive_actor(library: Arc<Library>, node: Arc<Node>, ingest_notify: Arc<Notify>) {
	let db = &library.db;
	let api_url = &library.env.api_url;
	let library_id = library.id;

	println!("receive_actor");

	let mut cloud_timestamps = {
		let timestamps = library.sync.timestamps.read().await;

		let batch = timestamps
			.keys()
			.map(|id| {
				db.cloud_shared_operation()
					.find_first(vec![cloud_shared_operation::instance::is(vec![
						instance::pub_id::equals(uuid_to_bytes(*id)),
					])])
					.order_by(cloud_shared_operation::timestamp::order(SortOrder::Desc))
			})
			.collect::<Vec<_>>();

		return_break!(db._batch(batch).await)
			.into_iter()
			.zip(timestamps.keys())
			.map(|(d, id)| {
				let cloud_timestamp = NTP64(d.map(|d| d.timestamp).unwrap_or_default() as u64);
				let sync_timestamp = *timestamps.get(id).unwrap();

				let max_timestamp = Ord::max(cloud_timestamp, sync_timestamp);

				(*id, max_timestamp)
			})
			.collect::<HashMap<_, _>>()
	};

	dbg!(&cloud_timestamps);

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
			start_time: String,
			end_time: String,
			contents: String,
		}

		{
			dbg!(&instances);

			let collections = err_break!(
				err_break!(
					node.authed_api_request(
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
				)
				.json::<Vec<MessageCollection>>()
				.await
			);

			dbg!(&collections);

			for collection in collections {
				if !cloud_timestamps.contains_key(&collection.instance_uuid) {
					err_break!(create_instance(&db, collection.instance_uuid).await);

					cloud_timestamps.insert(collection.instance_uuid, NTP64(0));
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

				let collection_timestamp = NTP64(collection.end_time.parse().unwrap());

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
	let (shared, relation): (Vec<_>, Vec<_>) = ops.into_iter().partition_map(|op| match &op.typ {
		CRDTOperationType::Shared(shared_op) => {
			Either::Left(shared_op_db(&op, &shared_op).to_query(&db))
		}
		CRDTOperationType::Relation(relation_op) => {
			Either::Right(relation_op_db(&op, &relation_op).to_query(&db))
		}
	});

	db._batch((shared, relation)).await?;

	Ok(())
}

fn shared_op_db(op: &CRDTOperation, shared_op: &SharedOperation) -> cloud_shared_operation::Create {
	cloud_shared_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: shared_op.kind().to_string(),
		data: to_vec(&shared_op.data).unwrap(),
		model: shared_op.model.to_string(),
		record_id: to_vec(&shared_op.record_id).unwrap(),
		_params: vec![],
	}
}

fn relation_op_db(
	op: &CRDTOperation,
	relation_op: &RelationOperation,
) -> cloud_relation_operation::Create {
	cloud_relation_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: relation_op.kind().to_string(),
		data: to_vec(&relation_op.data).unwrap(),
		relation: relation_op.relation.to_string(),
		item_id: to_vec(&relation_op.relation_item).unwrap(),
		group_id: to_vec(&relation_op.relation_group).unwrap(),
		_params: vec![],
	}
}

async fn create_instance(db: &PrismaClient, uuid: Uuid) -> prisma_client_rust::Result<()> {
	db.instance()
		.upsert(
			instance::pub_id::equals(uuid_to_bytes(uuid)),
			instance::create(
				uuid_to_bytes(uuid),
				vec![],
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

async fn ingest_actor(library: Arc<Library>, notify: Arc<Notify>) {
	let Library { sync, .. } = library.as_ref();

	loop {
		let mut rx = sync.ingest.req_rx.lock().await;

		sync.ingest
			.event_tx
			.send(sd_core_sync::Event::Notification)
			.await
			.unwrap();

		use crate::sync::ingest::*;

		while let Some(req) = rx.recv().await {
			const OPS_PER_REQUEST: u32 = 1000;

			let timestamps = match req {
				Request::FinishedIngesting => break,
				Request::Messages { timestamps } => timestamps,
				_ => continue,
			};

			let ops = sync
				.get_cloud_ops(crate::sync::GetOpsArgs {
					clocks: timestamps,
					count: 1000,
				})
				.await
				.unwrap();

			sync.ingest
				.event_tx
				.send(sd_core_sync::Event::Messages(MessagesEvent {
					instance_id: library.sync.instance,
					has_more: ops.len() == 1000,
					messages: ops,
				}))
				.await
				.unwrap();
		}

		notify.notified().await;
	}
}
