use std::sync::{atomic, Arc};
use tokio::sync::Notify;

use crate::{library::Library, Node};

pub async fn declare_actors(library: &Arc<Library>, node: &Arc<Node>) {
	let ingest_notify = Arc::new(Notify::new());
	let actors = &library.actors;

	let autorun = node.cloud_sync_flag.load(atomic::Ordering::Relaxed);

	actors
		.declare(
			"Cloud Sync Sender",
			{
				let library = library.clone();
				let node = node.clone();

				move || {
					sd_core_cloud_sync::send::run_actor(
						library.db.clone(),
						library.id,
						library.sync.clone(),
						node.clone(),
					)
				}
			},
			autorun,
		)
		.await;

	actors
		.declare(
			"Cloud Sync Receiver",
			{
				let library = library.clone();
				let node = node.clone();
				let ingest_notify = ingest_notify.clone();

				move || {
					sd_core_cloud_sync::receive::run_actor(
						library.db.clone(),
						library.id,
						library.instance_uuid,
						library.sync.clone(),
						node.clone(),
						ingest_notify,
					)
				}
			},
			autorun,
		)
		.await;

	actors
		.declare(
			"Cloud Sync Ingest",
			{
				let library = library.clone();
				move || sd_core_cloud_sync::ingest::run_actor(library.sync.clone(), ingest_notify)
			},
			autorun,
		)
		.await;
}
