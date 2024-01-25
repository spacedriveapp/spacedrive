use crate::cloud::sync::err_return;

use std::sync::Arc;

use tokio::sync::Notify;
use tracing::info;

use super::Library;

pub async fn run_actor((library, notify): (Arc<Library>, Arc<Notify>)) {
	let Library { sync, .. } = library.as_ref();

	loop {
		{
			let mut rx = sync.ingest.req_rx.lock().await;

			if sync
				.ingest
				.event_tx
				.send(sd_core_sync::Event::Notification)
				.await
				.is_ok()
			{
				use crate::sync::ingest::*;

				while let Some(req) = rx.recv().await {
					const OPS_PER_REQUEST: u32 = 1000;

					let timestamps = match req {
						Request::FinishedIngesting => break,
						Request::Messages { timestamps } => timestamps,
						_ => continue,
					};

					let ops = err_return!(
						sync.get_cloud_ops(crate::sync::GetOpsArgs {
							clocks: timestamps,
							count: OPS_PER_REQUEST,
						})
						.await
					);

					info!("Got {} cloud ops to ingest", ops.len());

					err_return!(
						sync.ingest
							.event_tx
							.send(sd_core_sync::Event::Messages(MessagesEvent {
								instance_id: library.sync.instance,
								has_more: ops.len() == 1000,
								messages: ops,
							}))
							.await
					);
				}
			}
		}

		notify.notified().await;
	}
}
