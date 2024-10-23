use crate::Error;

use sd_core_sync::SyncManager;

use sd_actors::{Actor, Stopper};

use std::{
	future::IntoFuture,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};

use futures::FutureExt;
use futures_concurrency::future::Race;
use tokio::{
	sync::Notify,
	time::{sleep, Instant},
};
use tracing::{debug, error};

use super::{ReceiveAndIngestNotifiers, SyncActors, ONE_MINUTE};

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

		loop {
			self.active.store(true, Ordering::Relaxed);
			self.active_notify.notify_waiters();

			if let Err(e) = self.run_loop_iteration().await {
				error!(?e, "Error during cloud sync ingester actor iteration");
				sleep(ONE_MINUTE).await;
				continue;
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

	async fn run_loop_iteration(&self) -> Result<(), Error> {
		let start = Instant::now();

		let operations_to_ingest_count = self
			.sync
			.db
			.cloud_crdt_operation()
			.count(vec![])
			.exec()
			.await
			.map_err(sd_core_sync::Error::from)?;

		if operations_to_ingest_count == 0 {
			debug!("Nothing to ingest, early finishing ingester loop");
			return Ok(());
		}

		debug!(
			operations_to_ingest_count,
			"Starting sync messages cloud ingestion loop"
		);

		let ingested_count = self.sync.ingest_ops().await?;

		debug!(
			ingested_count,
			"Finished sync messages cloud ingestion loop in {:?}",
			start.elapsed()
		);

		Ok(())
	}
}
