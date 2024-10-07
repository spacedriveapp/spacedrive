// use sd_core_sync::*;

// use sd_prisma::prisma;
// use sd_sync::CompressedCRDTOperationsPerModelPerDevice;

// use std::sync::{atomic::AtomicBool, Arc};

// use tokio::{fs, spawn, sync::broadcast};
// use tracing::{info, instrument, warn, Instrument};
// use uuid::Uuid;

// fn db_path(id: Uuid) -> String {
// 	format!("/tmp/test-{id}.db")
// }

// #[derive(Clone)]
// pub struct Device {
// 	pub pub_id: DevicePubId,
// 	pub db: Arc<prisma::PrismaClient>,
// 	pub sync: Arc<sd_core_sync::SyncManager>,
// 	pub sync_rx: Arc<broadcast::Receiver<SyncEvent>>,
// }

// impl Device {
// 	pub async fn new(id: Uuid) -> Arc<Self> {
// 		let url = format!("file:{}", db_path(id));
// 		let device_pub_id = DevicePubId::from(id);

// 		let db = Arc::new(
// 			prisma::PrismaClient::_builder()
// 				.with_url(url.to_string())
// 				.build()
// 				.await
// 				.unwrap(),
// 		);

// 		db._db_push().await.unwrap();

// 		db.device()
// 			.create(device_pub_id.to_db(), vec![])
// 			.exec()
// 			.await
// 			.unwrap();

// 		// let (sync, sync_rx) = sd_core_sync::SyncManager::new(
// 		// 	Arc::clone(&db),
// 		// 	&device_pub_id,
// 		// 	Arc::new(AtomicBool::new(true)),
// 		// 	Default::default(),
// 		// )
// 		// .await
// 		// .expect("failed to create sync manager");

// 		// Arc::new(Self {
// 		// 	pub_id: device_pub_id,
// 		// 	db,
// 		// 	sync: Arc::new(sync),
// 		// 	sync_rx: Arc::new(sync_rx),
// 		// })
// 	}

// 	pub async fn teardown(&self) {
// 		fs::remove_file(db_path(Uuid::from(&self.pub_id)))
// 			.await
// 			.unwrap();
// 	}

// 	pub async fn pair(instance1: &Arc<Self>, instance2: &Arc<Self>) {
// 		#[instrument(skip(left, right))]
// 		async fn half(left: &Arc<Device>, right: &Arc<Device>, context: &'static str) {
// 			left.db
// 				.device()
// 				.create(right.pub_id.to_db(), vec![])
// 				.exec()
// 				.await
// 				.unwrap();

// 			spawn({
// 				let mut sync_rx_left = left.sync_rx.resubscribe();
// 				let right = Arc::clone(right);

// 				async move {
// 					while let Ok(msg) = sync_rx_left.recv().await {
// 						info!(?msg, "sync_rx_left received message");
// 						if matches!(msg, SyncEvent::Created) {
// 							right
// 								.sync
// 								.ingest
// 								.event_tx
// 								.send(ingest::Event::Notification)
// 								.await
// 								.unwrap();
// 							info!("sent notification to instance 2");
// 						}
// 					}
// 				}
// 				.in_current_span()
// 			});

// 			spawn({
// 				let left = Arc::clone(left);
// 				let right = Arc::clone(right);

// 				async move {
// 					while let Ok(msg) = right.sync.ingest.req_rx.recv().await {
// 						info!(?msg, "right instance received request");
// 						match msg {
// 							ingest::Request::Messages { timestamps, tx } => {
// 								let messages = left.sync.get_ops(100, timestamps).await.unwrap();

// 								let ingest = &right.sync.ingest;

// 								ingest
// 									.event_tx
// 									.send(ingest::Event::Messages(ingest::MessagesEvent {
// 										messages: CompressedCRDTOperationsPerModelPerDevice::new(
// 											messages,
// 										),
// 										has_more: false,
// 										device_pub_id: left.pub_id.clone(),
// 										wait_tx: None,
// 									}))
// 									.await
// 									.unwrap();

// 								if tx.send(()).is_err() {
// 									warn!("failed to send ack to instance 1");
// 								}
// 							}
// 							ingest::Request::FinishedIngesting => {
// 								right.sync.tx.send(SyncEvent::Ingested).unwrap();
// 							}
// 						}
// 					}
// 				}
// 				.in_current_span()
// 			});
// 		}

// 		half(instance1, instance2, "instance1 -> instance2").await;
// 		half(instance2, instance1, "instance2 -> instance1").await;
// 	}
// }
