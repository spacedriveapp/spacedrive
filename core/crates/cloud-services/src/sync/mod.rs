use crate::CloudServices;

use sd_actors::ActorsCollection;
use sd_cloud_schema::sync::groups;
use sd_core_sync::SyncManager;

use sd_crypto::CryptoRng;
use sd_prisma::prisma::PrismaClient;

use std::{
	fmt,
	sync::{atomic::AtomicBool, Arc},
};

use tokio::sync::Notify;

pub mod ingest;
pub mod receive;
pub mod send;

#[derive(Default)]
pub struct SyncActorsState {
	pub send_active: Arc<AtomicBool>,
	pub receive_active: Arc<AtomicBool>,
	pub ingest_active: Arc<AtomicBool>,
	pub notifier: Arc<Notify>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, specta::Type)]
#[specta(rename = "CloudSyncActors")]
pub enum SyncActors {
	Ingester,
	Sender,
	Receiver,
}

impl fmt::Display for SyncActors {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Ingester => write!(f, "Cloud Sync Ingester"),
			Self::Sender => write!(f, "Cloud Sync Sender"),
			Self::Receiver => write!(f, "Cloud Sync Receiver"),
		}
	}
}

pub async fn declare_actors(
	cloud_services: Arc<CloudServices>,
	actors: &ActorsCollection<SyncActors>,
	actors_state: &SyncActorsState,
	sync_group_pub_id: groups::PubId,
	sync: SyncManager,
	db: Arc<PrismaClient>,
	rng: CryptoRng,
) {
	let ingest_notify = Arc::new(Notify::new());

	actors
		.declare(SyncActors::Sender, {
			let sync = sync.clone();
			let cloud_services = Arc::clone(&cloud_services);
			let active = Arc::clone(&actors_state.send_active);
			let active_notifier = Arc::clone(&actors_state.notifier);

			move |stop| {
				send::run_actor(
					sync_group_pub_id,
					sync,
					cloud_services,
					active,
					active_notifier,
					rng,
					stop,
				)
			}
		})
		.await;

	actors
		.declare(SyncActors::Receiver, {
			let sync = sync.clone();
			let cloud_services = cloud_services.clone();
			let db = Arc::clone(&db);
			let active = Arc::clone(&actors_state.receive_active);
			let ingest_notify = Arc::clone(&ingest_notify);
			let active_notifier = Arc::clone(&actors_state.notifier);

			move |stop| {
				receive::run_actor(
					db,
					sync_group_pub_id,
					cloud_services,
					sync,
					ingest_notify,
					(active, active_notifier),
					stop,
				)
			}
		})
		.await;

	// actors
	// 	.declare(
	// 		"Cloud Sync Ingest",
	// 		{
	// 			let active = state.ingest_active.clone();
	// 			let active_notifier = state.notifier.clone();

	// 			move |stop| {
	// 				ingest::run_actor(sync.clone(), ingest_notify, active, active_notifier, stop)
	// 			}
	// 		},
	// 		autorun,
	// 	)
	// 	.await;
}
