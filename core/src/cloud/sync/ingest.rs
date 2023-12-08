use std::sync::Arc;

use tokio::sync::Notify;

use super::Library;

pub async fn run_actor(library: Arc<Library>, notify: Arc<Notify>) {
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
					count: OPS_PER_REQUEST,
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
