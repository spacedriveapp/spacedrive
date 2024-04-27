use sd_sync::CompressedCRDTOperations;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc,
};
use tokio::sync::Notify;
use tracing::debug;

use crate::cloud::sync::err_break;

// Responsible for taking sync operations received from the cloud,
// and applying them to the local database via the sync system's ingest actor.

pub async fn run_actor(
	sync: Arc<sd_core_sync::Manager>,
	notify: Arc<Notify>,
	state: Arc<AtomicBool>,
	state_notify: Arc<Notify>,
) {
	loop {
		state.store(true, Ordering::Relaxed);
		state_notify.notify_waiters();

		{
			let mut rx = sync.ingest.req_rx.lock().await;

			if sync
				.ingest
				.event_tx
				.send(sd_core_sync::Event::Notification)
				.await
				.is_ok()
			{
				while let Some(req) = rx.recv().await {
					const OPS_PER_REQUEST: u32 = 1000;

					use sd_core_sync::*;

					let timestamps = match req {
						Request::FinishedIngesting => {
							break;
						}
						Request::Messages { timestamps, .. } => timestamps,
						_ => continue,
					};

					let ops = err_break!(
						sync.get_cloud_ops(GetOpsArgs {
							clocks: timestamps,
							count: OPS_PER_REQUEST,
						})
						.await
					);

					if ops.is_empty() {
						break;
					}

					debug!(
						"Sending {} messages ({:?} to {:?}) to ingester",
						ops.len(),
						ops.first().map(|operation| operation.timestamp.as_u64()),
						ops.last().map(|operation| operation.timestamp.as_u64()),
					);

					err_break!(
						sync.ingest
							.event_tx
							.send(sd_core_sync::Event::Messages(MessagesEvent {
								instance_id: sync.instance,
								has_more: ops.len() == OPS_PER_REQUEST as usize,
								messages: CompressedCRDTOperations::new(ops),
							}))
							.await
					);
				}
			}
		}

		state.store(false, Ordering::Relaxed);
		state_notify.notify_waiters();

		notify.notified().await;
	}
}
