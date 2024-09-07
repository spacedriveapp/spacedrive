use crate::Error;

use chrono::{DateTime, Utc};
use sd_core_sync::{from_cloud_crdt_ops, CompressedCRDTOperationsPerModelPerDevice, SyncManager};

use sd_cloud_schema::{devices, sync::groups};

use sd_actors::{Actor, Stopper};
use sd_prisma::prisma::{cloud_crdt_operation, SortOrder};

use std::{
	future::IntoFuture,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::SystemTime,
};

use futures::{FutureExt, StreamExt};
use futures_concurrency::future::Race;
use tokio::sync::Notify;
use tracing::debug;

use super::{timestamp_to_datetime, SyncActors};

const BATCH_SIZE: i64 = 1000;

/// Responsible for taking sync operations received from the cloud,
/// and applying them to the local database via the sync system's ingest actor.

pub struct Ingester {
	sync: SyncManager,
	ingest_notify: Arc<Notify>,
	active: Arc<AtomicBool>,
	active_notify: Arc<Notify>,
}

impl Actor<SyncActors> for Ingester {
	const IDENTIFIER: SyncActors = SyncActors::Ingester;

	async fn run(&mut self, stop: Stopper) {
		enum Race {
			Notified,
			Stopped,
		}

		loop {
			self.active.store(true, Ordering::Relaxed);
			self.active_notify.notify_waiters();

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

			self.active.store(false, Ordering::Relaxed);
			self.active_notify.notify_waiters();

			if matches!(
				(
					self.ingest_notify.notified().map(|()| Race::Notified),
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
}

enum IngestStatus {
	Completed,
	InProgress,
}

impl Ingester {
	pub fn new(
		sync: SyncManager,
		ingest_notify: Arc<Notify>,
		active: Arc<AtomicBool>,
		active_notify: Arc<Notify>,
	) -> Self {
		Self {
			sync,
			ingest_notify,
			active,
			active_notify,
		}
	}

	async fn run_loop_iteration(&self) -> Result<IngestStatus, Error> {
		let (ops_ids, ops) = self
			.sync
			.db
			.cloud_crdt_operation()
			.find_many(vec![])
			.take(BATCH_SIZE)
			.order_by(cloud_crdt_operation::timestamp::order(SortOrder::Asc))
			.exec()
			.await
			.map_err(sd_core_sync::Error::from)?
			.into_iter()
			.map(from_cloud_crdt_ops)
			.collect::<Result<(Vec<_>, Vec<_>), _>>()?;

		if ops_ids.is_empty() {
			return Ok(IngestStatus::Completed);
		}

		debug!(
			messages_count = ops.len(),
			first_message = ?ops
					.first()
					.map_or_else(|| SystemTime::UNIX_EPOCH.into(), |op| timestamp_to_datetime(op.timestamp)),
			last_message = ?ops
					.last()
					.map_or_else(|| SystemTime::UNIX_EPOCH.into(), |op| timestamp_to_datetime(op.timestamp)),
			"Messages to ingest",
		);

		let CompressedCRDTOperationsPerModelPerDevice(compressed_ops) =
			CompressedCRDTOperationsPerModelPerDevice::new(ops);

		self.sync
			.db
			.cloud_crdt_operation()
			.delete_many(vec![cloud_crdt_operation::id::in_vec(ops_ids)])
			.exec()
			.await
			.map_err(sd_core_sync::Error::from)?;

		Ok(IngestStatus::InProgress)
	}
}
