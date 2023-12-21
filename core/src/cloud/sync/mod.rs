use crate::{library::Library, Node};
use sd_sync::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use std::sync::{atomic, Arc};

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

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct CRDTOperationWithoutInstance {
	pub timestamp: NTP64,
	pub id: Uuid,
	pub model: String,
	pub record_id: Value,
	pub data: CRDTOperationData,
}

impl From<CRDTOperation> for CRDTOperationWithoutInstance {
	fn from(value: CRDTOperation) -> Self {
		Self {
			timestamp: value.timestamp,
			id: value.id,
			model: value.model,
			record_id: value.record_id,
			data: value.data,
		}
	}
}
