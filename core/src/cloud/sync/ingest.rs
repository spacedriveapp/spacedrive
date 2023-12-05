use super::Library;
use crate::Node;
use base64::prelude::*;
use chrono::Utc;
use itertools::{Either, Itertools};
use sd_core_sync::{GetOpsArgs, SyncMessage, NTP64};
use sd_sync::*;
use sd_utils::{from_bytes_to_uuid, uuid_to_bytes};
use serde::Deserialize;
use serde_json::{json, to_vec};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::Notify, time::sleep};
use uuid::Uuid;

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
