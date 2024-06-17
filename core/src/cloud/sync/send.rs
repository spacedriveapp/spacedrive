use sd_core_sync::{SyncMessage, NTP64};

use sd_cloud_api::RequestConfigProvider;

use std::{
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use tokio::{sync::Notify, time::sleep};
use tracing::debug;
use uuid::Uuid;

use super::{err_break, CompressedCRDTOperations};

pub async fn run_actor(
	library_id: Uuid,
	sync: Arc<sd_core_sync::Manager>,
	cloud_api_config_provider: Arc<impl RequestConfigProvider>,
	state: Arc<AtomicBool>,
	state_notify: Arc<Notify>,
) {
	loop {
		state.store(true, Ordering::Relaxed);
		state_notify.notify_waiters();

		loop {
			// all available instances will have a default timestamp from create_instance
			let instances = sync
				.timestamps
				.read()
				.await
				.keys()
				.cloned()
				.collect::<Vec<_>>();

			// obtains a lock on the timestamp collections for the instances we have
			let req_adds = err_break!(
				sd_cloud_api::library::message_collections::request_add(
					cloud_api_config_provider.get_request_config().await,
					library_id,
					instances,
				)
				.await
			);

			let mut instances = vec![];

			use sd_cloud_api::library::message_collections::do_add;

			debug!(
				total_operations = req_adds.len(),
				"Preparing to send instance's operations to cloud;"
			);

			// gets new operations for each instance to send to cloud
			for req_add in req_adds {
				let ops = err_break!(
					sync.get_instance_ops(
						1000,
						req_add.instance_uuid,
						NTP64(
							req_add
								.from_time
								.unwrap_or_else(|| "0".to_string())
								.parse()
								.expect("couldn't parse ntp64 value"),
						)
					)
					.await
				);

				if ops.is_empty() {
					continue;
				}

				let start_time = ops[0].timestamp.0.to_string();
				let end_time = ops[ops.len() - 1].timestamp.0.to_string();

				let ops_len = ops.len();

				use base64::prelude::*;

				debug!(instance_id = %req_add.instance_uuid, %start_time, %end_time);

				instances.push(do_add::Input {
					uuid: req_add.instance_uuid,
					key: req_add.key,
					start_time,
					end_time,
					contents: BASE64_STANDARD.encode(
						rmp_serde::to_vec_named(&CompressedCRDTOperations::new(ops))
							.expect("CompressedCRDTOperation should serialize!"),
					),
					ops_count: ops_len,
				})
			}

			if instances.is_empty() {
				break;
			}

			// uses lock we acquired earlier to send the operations to the cloud
			err_break!(
				do_add(
					cloud_api_config_provider.get_request_config().await,
					library_id,
					instances,
				)
				.await
			);
		}

		state.store(false, Ordering::Relaxed);
		state_notify.notify_waiters();

		{
			// recreate subscription each time so that existing messages are dropped
			let mut rx = sync.subscribe();

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
