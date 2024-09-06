use sd_core_sync::SyncManager;

use sd_actors::Stopper;

use std::{
	future::IntoFuture,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use futures::{FutureExt, StreamExt};
use futures_concurrency::future::Race;
use tokio::sync::Notify;

// Responsible for taking sync operations received from the cloud,
// and applying them to the local database via the sync system's ingest actor.

pub async fn run_actor(
	sync: SyncManager,
	notify: Arc<Notify>,
	state: Arc<AtomicBool>,
	state_notify: Arc<Notify>,
	stop: Stopper,
) {
	enum Race {
		Notified,
		Stopped,
	}

	loop {
		state.store(true, Ordering::Relaxed);
		state_notify.notify_waiters();

		// 	{
		// 		let mut rx = pin!(sync.ingest.req_rx.clone());

		// 		if sync
		// 			.ingest
		// 			.event_tx
		// 			.send(sd_core_sync::Event::Notification)
		// 			.await
		// 			.is_ok()
		// 		{
		// 			while let Some(req) = rx.next().await {
		// 				const OPS_PER_REQUEST: u32 = 1000;

		// 				// FIXME: If there are exactly a multiple of OPS_PER_REQUEST operations,
		// 				// then this will bug, as we sent `has_more` as true, but we don't have
		// 				// more operations to send.

		// 				use sd_core_sync::*;

		// 				let timestamps = match req {
		// 					Request::FinishedIngesting => {
		// 						break;
		// 					}
		// 					Request::Messages { timestamps, .. } => timestamps,
		// 				};

		// 				let (ops_ids, ops): (Vec<_>, Vec<_>) =
		// 					err_break!(sync.get_cloud_ops(OPS_PER_REQUEST, timestamps,).await)
		// 						.into_iter()
		// 						.unzip();

		// 				if ops.is_empty() {
		// 					break;
		// 				}

		// 				debug!(
		// 					messages_count = ops.len(),
		// 					first_message = ?ops.first().map(|operation| operation.timestamp.as_u64()),
		// 					last_message = ?ops.last().map(|operation| operation.timestamp.as_u64()),
		// 					"Sending messages to ingester",
		// 				);

		// 				let (wait_tx, wait_rx) = tokio::sync::oneshot::channel::<()>();

		// 				err_break!(
		// 					sync.ingest
		// 						.event_tx
		// 						.send(sd_core_sync::Event::Messages(MessagesEvent {
		// 							device_pub_id: sync.device_pub_id.clone(),
		// 							has_more: ops.len() == OPS_PER_REQUEST as usize,
		// 							messages: CompressedCRDTOperationsPerModelPerDevice::new(ops),
		// 							wait_tx: Some(wait_tx)
		// 						}))
		// 						.await
		// 				);

		// 				err_break!(wait_rx.await);

		// 				err_break!(
		// 					sync.db
		// 						.cloud_crdt_operation()
		// 						.delete_many(vec![cloud_crdt_operation::id::in_vec(ops_ids)])
		// 						.exec()
		// 						.await
		// 				);
		// 			}
		// 		}
		// 	}

		state.store(false, Ordering::Relaxed);
		state_notify.notify_waiters();

		if matches!(
			(
				notify.notified().map(|()| Race::Notified),
				stop.into_future().map(|()| Race::Stopped),
			)
				.race()
				.await,
			Race::Stopped
		) {
			break;
		}
	}
}
