use sd_core_sync::*;

use sd_prisma::prisma;
use sd_sync::CompressedCRDTOperations;
use sd_utils::uuid_to_bytes;

use std::sync::{atomic::AtomicBool, Arc};

use prisma_client_rust::chrono::Utc;
use tokio::{fs, spawn, sync::broadcast};
use tracing::{info, instrument, warn, Instrument};
use uuid::Uuid;

fn db_path(id: Uuid) -> String {
	format!("/tmp/test-{id}.db")
}

#[derive(Clone)]
pub struct Instance {
	pub id: Uuid,
	pub db: Arc<prisma::PrismaClient>,
	pub sync: Arc<sd_core_sync::Manager>,
	pub sync_rx: Arc<broadcast::Receiver<SyncMessage>>,
}

impl Instance {
	pub async fn new(id: Uuid) -> Arc<Self> {
		let url = format!("file:{}", db_path(id));

		let db = Arc::new(
			prisma::PrismaClient::_builder()
				.with_url(url.to_string())
				.build()
				.await
				.unwrap(),
		);

		db._db_push().await.unwrap();

		db.instance()
			.create(
				uuid_to_bytes(&id),
				vec![],
				vec![],
				Utc::now().into(),
				Utc::now().into(),
				vec![],
			)
			.exec()
			.await
			.unwrap();

		let (sync, sync_rx) = sd_core_sync::Manager::new(
			Arc::clone(&db),
			id,
			Arc::new(AtomicBool::new(true)),
			Default::default(),
		)
		.await
		.expect("failed to create sync manager");

		Arc::new(Self {
			id,
			db,
			sync: Arc::new(sync),
			sync_rx: Arc::new(sync_rx),
		})
	}

	pub async fn teardown(&self) {
		fs::remove_file(db_path(self.id)).await.unwrap();
	}

	pub async fn pair(instance1: &Arc<Self>, instance2: &Arc<Self>) {
		#[instrument(skip(left, right))]
		async fn half(left: &Arc<Instance>, right: &Arc<Instance>, context: &'static str) {
			left.db
				.instance()
				.create(
					uuid_to_bytes(&right.id),
					vec![],
					vec![],
					Utc::now().into(),
					Utc::now().into(),
					vec![],
				)
				.exec()
				.await
				.unwrap();

			spawn({
				let mut sync_rx_left = left.sync_rx.resubscribe();
				let right = Arc::clone(right);

				async move {
					while let Ok(msg) = sync_rx_left.recv().await {
						info!(?msg, "sync_rx_left received message");
						if matches!(msg, SyncMessage::Created) {
							right
								.sync
								.ingest
								.event_tx
								.send(ingest::Event::Notification)
								.await
								.unwrap();
							info!("sent notification to instance 2");
						}
					}
				}
				.in_current_span()
			});

			spawn({
				let left = Arc::clone(left);
				let right = Arc::clone(right);

				async move {
					while let Ok(msg) = right.sync.ingest.req_rx.recv().await {
						info!(?msg, "right instance received request");
						match msg {
							ingest::Request::Messages { timestamps, tx } => {
								let messages = left
									.sync
									.get_ops(GetOpsArgs {
										clocks: timestamps,
										count: 100,
									})
									.await
									.unwrap();

								let ingest = &right.sync.ingest;

								ingest
									.event_tx
									.send(ingest::Event::Messages(ingest::MessagesEvent {
										messages: CompressedCRDTOperations::new(messages),
										has_more: false,
										instance_id: left.id,
										wait_tx: None,
									}))
									.await
									.unwrap();

								if tx.send(()).is_err() {
									warn!("failed to send ack to instance 1");
								}
							}
							ingest::Request::FinishedIngesting => {
								right.sync.tx.send(SyncMessage::Ingested).ok();
							}
						}
					}
				}
				.in_current_span()
			});
		}

		half(instance1, instance2, "instance1 -> instance2").await;
		half(instance2, instance1, "instance2 -> instance1").await;
	}
}
