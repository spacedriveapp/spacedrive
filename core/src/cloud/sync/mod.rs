use std::sync::{atomic, Arc};

use crate::{library::Library, Node};

mod ingest;
mod receive;
mod send;

pub async fn declare_actors(library: &Arc<Library>, node: &Arc<Node>) {
	let ingest_notify = Arc::new(Notify::new());
	let actors = &library.actors;

	let autorun = node.cloud_sync_flag.load(atomic::Ordering::Relaxed);

	let args = (library.clone(), node.clone());
	actors
		.declare("Cloud Sync Sender", move || send::run_actor(args), autorun)
		.await;

	let args = (library.clone(), node.clone(), ingest_notify.clone());
	actors
		.declare(
			"Cloud Sync Receiver",
			move || receive::run_actor(args),
			autorun,
		)
		.await;

	let args = (library.clone(), ingest_notify);
	actors
		.declare(
			"Cloud Sync Ingest",
			move || ingest::run_actor(args),
			autorun,
		)
		.await;
}

macro_rules! err_break {
	($e:expr) => {
		match $e {
			Ok(d) => d,
			Err(e) => {
				tracing::error!("{e}");
				break;
			}
		}
	};
}
pub(crate) use err_break;

macro_rules! err_return {
	($e:expr) => {
		match $e {
			Ok(d) => d,
			Err(e) => {
				tracing::error!("{e}");
				return;
			}
		}
	};
}

pub(crate) use err_return;
use tokio::sync::Notify;
