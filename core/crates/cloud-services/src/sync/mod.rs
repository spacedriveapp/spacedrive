use crate::{CloudServices, Error};

use futures_concurrency::future::TryJoin;
use sd_core_sync::{SyncManager, NTP64};

use sd_actors::{ActorsCollection, IntoActor};
use sd_cloud_schema::sync::groups;
use sd_crypto::CryptoRng;

use std::{
	fmt,
	path::Path,
	sync::{atomic::AtomicBool, Arc},
	time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Utc};
use tokio::sync::Notify;

mod ingest;
mod receive;
mod send;

// use ingest::Ingester;
use receive::Receiver;
use send::Sender;

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
	data_dir: Box<Path>,
	cloud_services: Arc<CloudServices>,
	actors: &ActorsCollection<SyncActors>,
	actors_state: &SyncActorsState,
	sync_group_pub_id: groups::PubId,
	sync: SyncManager,
	rng: CryptoRng,
) -> Result<(), Error> {
	let ingest_notify = Arc::new(Notify::new());

	let (sender, receiver) = (
		Sender::new(
			sync_group_pub_id,
			sync.clone(),
			Arc::clone(&cloud_services),
			Arc::clone(&actors_state.send_active),
			Arc::clone(&actors_state.notifier),
			rng,
		),
		Receiver::new(
			data_dir,
			sync_group_pub_id,
			cloud_services,
			sync.clone(),
			Arc::clone(&ingest_notify),
			Arc::clone(&actors_state.receive_active),
			Arc::clone(&actors_state.notifier),
		),
	)
		.try_join()
		.await?;

	// let ingester = Ingester::new(
	// 	sync,
	// 	ingest_notify,
	// 	Arc::clone(&actors_state.ingest_active),
	// 	Arc::clone(&actors_state.notifier),
	// );

	actors
		.declare_many_boxed([
			sender.into_actor(),
			receiver.into_actor(),
			// ingester.into_actor(),
		])
		.await;

	Ok(())
}

fn datetime_to_timestamp(latest_time: DateTime<Utc>) -> NTP64 {
	NTP64::from(
		SystemTime::from(latest_time)
			.duration_since(UNIX_EPOCH)
			.expect("hardcoded earlier time, nothing is earlier than UNIX_EPOCH"),
	)
}

fn timestamp_to_datetime(timestamp: NTP64) -> DateTime<Utc> {
	DateTime::from(timestamp.to_system_time())
}
