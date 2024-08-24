use sd_actors::Stopper;
use sd_core_cloud_services::CloudServices;
use sd_core_sync::SyncMessage;

use std::{
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};

use tokio::{
	sync::{broadcast, Notify},
	time::sleep,
};
use uuid::Uuid;

enum RaceNotifiedOrStopped {
	Notified,
	Stopped,
}

pub async fn run_actor(
	library_id: Uuid,
	sync: Arc<sd_core_sync::Manager>,
	cloud_services: CloudServices,
	is_active: Arc<AtomicBool>,
	state_notify: Arc<Notify>,
	stop: Stopper,
) {
	loop {
		is_active.store(true, Ordering::Relaxed);
		state_notify.notify_waiters();

		// loop {
		// 	// all available instances will have a default timestamp from create_instance
		// 	let instances = sync
		// 		.timestamp_per_device
		// 		.read()
		// 		.await
		// 		.keys()
		// 		.cloned()
		// 		.collect::<Vec<_>>();

		// 	// obtains a lock on the timestamp collections for the instances we have

		// 	debug!(
		// 		total_operations = req_adds.len(),
		// 		"Preparing to send instance's operations to cloud;"
		// 	);

		// 	// gets new operations for each instance to send to cloud
		// 	for req_add in req_adds {
		// 		let ops = err_break!(
		// 			sync.get_instance_ops(
		// 				1000,
		// 				req_add.instance_uuid,
		// 				NTP64(
		// 					req_add
		// 						.from_time
		// 						.unwrap_or_else(|| "0".to_string())
		// 						.parse()
		// 						.expect("couldn't parse ntp64 value"),
		// 				)
		// 			)
		// 			.await
		// 		);

		// 		if ops.is_empty() {
		// 			continue;
		// 		}

		// 		let start_time = ops[0].timestamp.0.to_string();
		// 		let end_time = ops[ops.len() - 1].timestamp.0.to_string();

		// 		let ops_len = ops.len();

		// 		use base64::prelude::*;

		// 		debug!(instance_id = %req_add.instance_uuid, %start_time, %end_time);

		// 		instances.push(do_add::Input {
		// 			uuid: req_add.instance_uuid,
		// 			key: req_add.key,
		// 			start_time,
		// 			end_time,
		// 			contents: BASE64_STANDARD.encode(
		// 				rmp_serde::to_vec_named(&CompressedCRDTOperations::new(ops))
		// 					.expect("CompressedCRDTOperation should serialize!"),
		// 			),
		// 			ops_count: ops_len,
		// 		})
		// 	}

		// 	if instances.is_empty() {
		// 		break;
		// 	}

		// 	// uses lock we acquired earlier to send the operations to the cloud
		// 	err_break!(
		// 		do_add(
		// 			cloud_api_config_provider.get_request_config().await,
		// 			library_id,
		// 			instances,
		// 		)
		// 		.await
		// 	);
		// }

		// is_active.store(false, Ordering::Relaxed);
		// state_notify.notify_waiters();

		// if let RaceNotifiedOrStopped::Stopped = (
		// 	// recreate subscription each time so that existing messages are dropped
		// 	wait_notification(sync.subscribe()),
		// 	stop.into_future().map(|()| RaceNotifiedOrStopped::Stopped),
		// )
		// 	.race()
		// 	.await
		// {
		// 	break;
		// }

		sleep(Duration::from_millis(1000)).await;
	}
}

async fn wait_notification(mut rx: broadcast::Receiver<SyncMessage>) -> RaceNotifiedOrStopped {
	// wait until Created message comes in
	loop {
		if let Ok(SyncMessage::Created) = rx.recv().await {
			break;
		};
	}

	RaceNotifiedOrStopped::Notified
}
