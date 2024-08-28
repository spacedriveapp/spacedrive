use crate::Node;

use sd_core_sync::SyncManager;

use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::Notify;
use uuid::Uuid;

pub mod ingest;
pub mod receive;
pub mod send;

#[derive(Default)]
pub struct State {
	pub send_active: Arc<AtomicBool>,
	pub receive_active: Arc<AtomicBool>,
	pub ingest_active: Arc<AtomicBool>,
	pub notifier: Arc<Notify>,
}

pub async fn declare_actors(
	node: &Arc<Node>,
	actors: &Arc<sd_actors::Actors>,
	library_id: Uuid,
	instance_uuid: Uuid,
	sync: Arc<SyncManager>,
	db: Arc<sd_prisma::prisma::PrismaClient>,
) -> State {
	let ingest_notify = Arc::new(Notify::new());

	// actors
	// 	.declare(
	// 		"Cloud Sync Sender",
	// 		{
	// 			let sync = sync.clone();
	// 			let node = node.clone();
	// 			let active = state.send_active.clone();
	// 			let active_notifier = state.notifier.clone();

	// 			move |stop| send::run_actor(library_id, sync, node, active, active_notifier, stop)
	// 		},
	// 		autorun,
	// 	)
	// 	.await;

	// actors
	// 	.declare(
	// 		"Cloud Sync Receiver",
	// 		{
	// 			let sync = sync.clone();
	// 			let node = node.clone();
	// 			let ingest_notify = ingest_notify.clone();
	// 			let active_notifier = state.notifier.clone();
	// 			let active = state.receive_active.clone();

	// 			move |stop| {
	// 				receive::run_actor(
	// 					node.libraries.clone(),
	// 					db.clone(),
	// 					library_id,
	// 					instance_uuid,
	// 					sync,
	// 					ingest_notify,
	// 					node,
	// 					active,
	// 					active_notifier,
	// 					stop,
	// 				)
	// 			}
	// 		},
	// 		autorun,
	// 	)
	// 	.await;

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

	State::default()
}

macro_rules! err_break {
	($e:expr) => {
		match $e {
			Ok(d) => d,
			Err(e) => {
				tracing::error!(?e);
				break;
			}
		}
	};
}
pub(crate) use err_break;
