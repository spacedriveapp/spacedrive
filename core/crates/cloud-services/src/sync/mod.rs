use crate::{CloudServices, Error};

use sd_core_sync::SyncManager;

use sd_actors::{ActorsCollection, IntoActor};
use sd_cloud_schema::sync::groups;
use sd_crypto::CryptoRng;

use std::{
	fmt,
	path::Path,
	sync::{atomic::AtomicBool, Arc},
	time::Duration,
};

use futures_concurrency::future::TryJoin;
use tokio::sync::Notify;

mod ingest;
mod receive;
mod send;

use ingest::Ingester;
use receive::Receiver;
use send::Sender;

const ONE_MINUTE: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct SyncActorsState {
	pub send_active: Arc<AtomicBool>,
	pub receive_active: Arc<AtomicBool>,
	pub ingest_active: Arc<AtomicBool>,
	pub state_change_notifier: Arc<Notify>,
	receiver_and_ingester_notifiers: Arc<ReceiveAndIngestNotifiers>,
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

#[derive(Debug, Default)]
pub struct ReceiveAndIngestNotifiers {
	ingester: Notify,
	receiver: Notify,
}

impl ReceiveAndIngestNotifiers {
	pub fn notify_receiver(&self) {
		self.receiver.notify_one();
	}

	async fn wait_notification_to_receive(&self) {
		self.receiver.notified().await;
	}

	fn notify_ingester(&self) {
		self.ingester.notify_one();
	}

	async fn wait_notification_to_ingest(&self) {
		self.ingester.notified().await;
	}
}

pub async fn declare_actors(
	data_dir: Box<Path>,
	cloud_services: Arc<CloudServices>,
	actors: &ActorsCollection<SyncActors>,
	actors_state: &SyncActorsState,
	sync_group_pub_id: groups::PubId,
	sync: SyncManager,
	rng: CryptoRng,
) -> Result<Arc<ReceiveAndIngestNotifiers>, Error> {
	let (sender, receiver) = (
		Sender::new(
			sync_group_pub_id,
			sync.clone(),
			Arc::clone(&cloud_services),
			Arc::clone(&actors_state.send_active),
			Arc::clone(&actors_state.state_change_notifier),
			rng,
		),
		Receiver::new(
			data_dir,
			sync_group_pub_id,
			cloud_services.clone(),
			sync.clone(),
			Arc::clone(&actors_state.receiver_and_ingester_notifiers),
			Arc::clone(&actors_state.receive_active),
			Arc::clone(&actors_state.state_change_notifier),
		),
	)
		.try_join()
		.await?;

	let ingester = Ingester::new(
		sync,
		Arc::clone(&actors_state.receiver_and_ingester_notifiers),
		Arc::clone(&actors_state.ingest_active),
		Arc::clone(&actors_state.state_change_notifier),
	);

	actors
		.declare_many_boxed([
			sender.into_actor(),
			receiver.into_actor(),
			ingester.into_actor(),
		])
		.await;

	cloud_services
		.cloud_p2p()
		.await?
		.register_sync_messages_receiver_notifier(
			sync_group_pub_id,
			Arc::clone(&actors_state.receiver_and_ingester_notifiers),
		)
		.await;

	Ok(Arc::clone(&actors_state.receiver_and_ingester_notifiers))
}
