use crate::Error;

use sd_core_sync::{from_cloud_crdt_ops, CompressedCRDTOperationsPerModelPerDevice, SyncManager};

use sd_actors::{Actor, Stopper};
use sd_prisma::prisma::{cloud_crdt_operation, SortOrder};
use sd_utils::timestamp_to_datetime;

use std::{
	future::IntoFuture,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::SystemTime,
};

use futures::FutureExt;
use futures_concurrency::future::Race;
use tokio::{sync::Notify, time::sleep};
use tracing::{debug, error};

use super::{ReceiveAndIngestNotifiers, SyncActors, ONE_MINUTE};

const BATCH_SIZE: i64 = 1000;

/// Responsible for taking sync operations received from the cloud,
/// and applying them to the local database via the sync system's ingest actor.

pub struct Ingester {
	sync: SyncManager,
	notifiers: Arc<ReceiveAndIngestNotifiers>,
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

		'outer: loop {
			self.active.store(true, Ordering::Relaxed);
			self.active_notify.notify_waiters();

			loop {
				match self.run_loop_iteration().await {
					Ok(IngestStatus::Completed) => break,
					Ok(IngestStatus::InProgress) => {}
					Err(e) => {
						error!(?e, "Error during cloud sync ingester actor iteration");
						sleep(ONE_MINUTE).await;
						continue 'outer;
					}
				}
			}

			self.active.store(false, Ordering::Relaxed);
			self.active_notify.notify_waiters();

			if matches!(
				(
					self.notifiers
						.wait_notification_to_ingest()
						.map(|()| Race::Notified),
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
	pub const fn new(
		sync: SyncManager,
		notifiers: Arc<ReceiveAndIngestNotifiers>,
		active: Arc<AtomicBool>,
		active_notify: Arc<Notify>,
	) -> Self {
		Self {
			sync,
			notifiers,
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

		self.sync
			.ingest_ops(CompressedCRDTOperationsPerModelPerDevice::new(ops))
			.await?;

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
