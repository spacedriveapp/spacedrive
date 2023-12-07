use std::sync::Arc;

use crate::{library::Library, Node};

mod ingest;
mod receive;
mod send;

pub fn spawn_actors(library: &Arc<Library>, node: &Arc<Node>) {
	let ingest_notify = Arc::new(Notify::new());

	tokio::spawn(send::run_actor(library.clone(), node.clone()));
	tokio::spawn(receive::run_actor(
		library.clone(),
		node.clone(),
		ingest_notify.clone(),
	));
	tokio::spawn(ingest::run_actor(library.clone(), ingest_notify));
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

macro_rules! return_break {
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

pub(crate) use return_break;
use tokio::sync::Notify;
