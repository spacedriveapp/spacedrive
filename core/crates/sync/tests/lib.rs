use sd_core_sync::*;
use sd_prisma::{prisma, prisma_sync};
use sd_sync::*;
use sd_utils::uuid_to_bytes;

use prisma_client_rust::chrono::Utc;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

fn db_path(id: Uuid) -> String {
	format!("./tests/test-{id}.db")
}

#[derive(Clone)]
struct Instance {
	id: Uuid,
	_peer_id: sd_p2p::PeerId,
	db: Arc<prisma::PrismaClient>,
	sync: Arc<sd_core_sync::Manager>,
}

impl Instance {
	async fn new(
		id: Uuid,
	) -> (
		Arc<Self>,
		broadcast::Receiver<SyncMessage>,
		mpsc::Receiver<ingest::Request>,
	) {
		let db = Arc::new(
			prisma::PrismaClient::_builder()
				.with_url(format!("file:{}", db_path(id)))
				.build()
				.await
				.unwrap(),
		);

		db._db_push().await.unwrap();

		db.instance()
			.create(
				uuid_to_bytes(id),
				vec![],
				vec![],
				format!("Instace {id}"),
				0,
				Utc::now().into(),
				Utc::now().into(),
				vec![],
			)
			.exec()
			.await
			.unwrap();

		let sync = sd_core_sync::Manager::new(&db, id);

		(
			Arc::new(Self {
				id,
				db,
				_peer_id: sd_p2p::PeerId::random(),
				sync: Arc::new(sync.manager),
			}),
			sync.rx,
			sync.ingest_rx,
		)
	}

	async fn teardown(&self) {
		tokio::fs::remove_file(db_path(self.id)).await.unwrap();
	}

	async fn pair(left: &Self, right: &Self) {
		left.db
			.instance()
			.create(
				uuid_to_bytes(right.id),
				vec![],
				vec![],
				"".to_string(),
				0,
				Utc::now().into(),
				Utc::now().into(),
				vec![],
			)
			.exec()
			.await
			.unwrap();

		right
			.db
			.instance()
			.create(
				uuid_to_bytes(left.id),
				vec![],
				vec![],
				"".to_string(),
				0,
				Utc::now().into(),
				Utc::now().into(),
				vec![],
			)
			.exec()
			.await
			.unwrap();
	}
}

#[tokio::test]
async fn bruh() -> Result<(), Box<dyn std::error::Error>> {
	let (instance1, mut sync_rx1, _) = Instance::new(Uuid::new_v4()).await;
	let (instance2, _, mut ingest_rx2) = Instance::new(Uuid::new_v4()).await;

	Instance::pair(&instance1, &instance2).await;

	tokio::spawn({
		let _instance1 = instance1.clone();
		let instance2 = instance2.clone();

		async move {
			while let Ok(msg) = sync_rx1.recv().await {
				match msg {
					SyncMessage::Created => {
						instance2.sync.ingest.event_tx.send(todo!()).await.unwrap()
					}
					_ => {}
				}
			}
		}
	});

	tokio::spawn({
		let instance1 = instance1.clone();
		let instance2 = instance2.clone();

		async move {
			while let Some(msg) = ingest_rx2.recv().await {
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

						instance2
							.sync
							.ingest
							.event_tx
							.send(ingest::Event::Messages(ingest::MessagesEvent {
								tunnel: todo!(),
								messages,
								has_more: false,
								instance_id: instance1.id,
							}))
							.await
							.unwrap();
					}
					_ => {}
				}
			}
		}
	});

	instance1
		.sync
		.write_ops(&instance1.db, {
			let id = Uuid::new_v4();

			use prisma::location;

			macro_rules! item {
				($name:ident, $value:expr) => {
					(
						(location::$name::NAME, json!($value)),
						location::$name::set(Some($value.to_string())),
					)
				};
			}

			let (sync_ops, db_ops): (Vec<_>, Vec<_>) = [
				item!(name, "Location 0"),
				item!(path, "/User/Brendan/Documents"),
			]
			.into_iter()
			.unzip();

			(
				instance1.sync.shared_create(
					prisma_sync::location::SyncId {
						pub_id: uuid_to_bytes(id),
					},
					sync_ops,
				),
				instance1.db.location().create(uuid_to_bytes(id), db_ops),
			)
		})
		.await?;

	tokio::time::sleep(Duration::from_millis(10)).await;

	// assert_eq!(out.len(), 3);
	// assert!(matches!(out[0].typ, CRDTOperationType::Shared(_)));

	instance1.teardown().await;
	instance2.teardown().await;

	Ok(())
}
