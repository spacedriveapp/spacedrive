use super::Library;
use crate::{cloud::sync::err_break, Node};
use sd_core_sync::{GetOpsArgs, SyncMessage, NTP64};
use sd_prisma::prisma::instance;
use sd_utils::from_bytes_to_uuid;
use serde::Deserialize;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use uuid::Uuid;

pub async fn run_actor((library, node): (Arc<Library>, Arc<Node>)) {
	let db = &library.db;
	let api_url = &library.env.api_url;
	let library_id = library.id;

	loop {
		loop {
			let instances = err_break!(
				db.instance()
					.find_many(vec![])
					.select(instance::select!({ pub_id }))
					.exec()
					.await
			)
			.into_iter()
			.map(|i| json!({ "instanceUuid": from_bytes_to_uuid(&i.pub_id).to_string() }))
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

			let mut instances = vec![];

			for req_add in req_adds {
				let ops = err_break!(
					library
						.sync
						.get_ops(GetOpsArgs {
							count: 1000,
							clocks: vec![(
								req_add.instance_uuid,
								NTP64(
									req_add
										.from_time
										.unwrap_or_else(|| "0".to_string())
										.parse()
										.expect("couldn't parse ntp64 value"),
								),
							)],
						})
						.await
				);

				if ops.is_empty() {
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
					.map(|i| i["contents"].as_array().expect("no contents found").len())
					.sum::<usize>()
			);

			if instances.is_empty() {
				break;
			}

			#[derive(Deserialize, Debug)]
			#[serde(rename_all = "camelCase")]
			struct DoAdd {
				// instance_uuid: Uuid,
				// from_time: String,
			}

			let _responses = err_break!(
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
		}

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

		sleep(Duration::from_millis(1000)).await;
	}
}
