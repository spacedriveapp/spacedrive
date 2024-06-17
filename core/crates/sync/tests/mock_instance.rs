use sd_core_sync::*;
use sd_prisma::prisma::{self};
use sd_sync::CompressedCRDTOperations;
use sd_utils::uuid_to_bytes;

use prisma_client_rust::chrono::Utc;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::broadcast;
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

		let sync = sd_core_sync::Manager::new(
			&db,
			id,
			&Arc::new(AtomicBool::new(true)),
			Default::default(),
			&Default::default(),
		)
		.await;

		Arc::new(Self {
			id,
			db,
			sync: Arc::new(sync.manager),
			sync_rx: Arc::new(sync.rx),
		})
	}

	pub async fn teardown(&self) {
		tokio::fs::remove_file(db_path(self.id)).await.unwrap();
	}

	pub async fn pair(left: &Arc<Self>, right: &Arc<Self>) {
		async fn half(left: &Arc<Instance>, right: &Arc<Instance>) {
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

			tokio::spawn({
				let mut sync_rx_1 = left.sync_rx.resubscribe();
				let instance2 = right.clone();

				async move {
					while let Ok(msg) = sync_rx_1.recv().await {
						if matches!(msg, SyncMessage::Created) {
							instance2
								.sync
								.ingest
								.event_tx
								.send(ingest::Event::Notification)
								.await
								.unwrap();
						}
					}
				}
			});

			tokio::spawn({
				let instance1 = left.clone();
				let instance2 = right.clone();

				async move {
					while let Some(msg) = instance2.sync.ingest.req_rx.lock().await.recv().await {
						match msg {
							ingest::Request::Messages { timestamps, .. } => {
								let messages = instance1
									.sync
									.get_ops(GetOpsArgs {
										clocks: timestamps,
										count: 100,
									})
									.await
									.unwrap();

								let ingest = &instance2.sync.ingest;

								ingest
									.event_tx
									.send(ingest::Event::Messages(ingest::MessagesEvent {
										messages: CompressedCRDTOperations::new(messages),
										has_more: false,
										instance_id: instance1.id,
										wait_tx: None,
									}))
									.await
									.unwrap();
							}
							// ingest::Request::Ingested => {
							// 	instance2.sync.tx.send(SyncMessage::Ingested).ok();
							// }
							ingest::Request::FinishedIngesting => {}
						}
					}
				}
			});
		}

		half(left, right).await;
		half(right, left).await;
	}
}
