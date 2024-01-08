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

#[derive(Serialize, Deserialize)]
pub struct CompressedCRDTOperations(
	Vec<(
		Uuid,
		Vec<(String, Vec<(Value, Vec<CompressedCRDTOperation>)>)>,
	)>,
);

impl CompressedCRDTOperations {
	pub fn new(ops: Vec<CRDTOperation>) -> Self {
		let mut compressed = vec![];

		let mut ops_iter = ops.into_iter();

		let Some(first) = ops_iter.next() else {
			return Self(vec![]);
		};

		let mut instance_id = first.instance;
		let mut instance = vec![];

		let mut model_str = first.model.clone();
		let mut model = vec![];

		let mut record_id = first.record_id.clone();
		let mut record = vec![first.into()];

		for op in ops_iter {
			if instance_id != op.instance {
				model.push((
					std::mem::replace(&mut record_id, op.record_id.clone()),
					std::mem::take(&mut record),
				));
				instance.push((
					std::mem::replace(&mut model_str, op.model.clone()),
					std::mem::take(&mut model),
				));
				compressed.push((
					std::mem::replace(&mut instance_id, op.instance),
					std::mem::take(&mut instance),
				));
			} else if model_str != op.model {
				model.push((
					std::mem::replace(&mut record_id, op.record_id.clone()),
					std::mem::take(&mut record),
				));
				instance.push((
					std::mem::replace(&mut model_str, op.model.clone()),
					std::mem::take(&mut model),
				));
			} else if record_id != op.record_id {
				model.push((
					std::mem::replace(&mut record_id, op.record_id.clone()),
					std::mem::take(&mut record),
				));
			}

			record.push(CompressedCRDTOperation::from(op))
		}

		model.push((record_id, record));
		instance.push((model_str, model));
		compressed.push((instance_id, instance));

		Self(compressed)
	}

	pub fn to_ops(self) -> Vec<CRDTOperation> {
		let mut ops = vec![];

		for (instance_id, instance) in self.0 {
			for (model_str, model) in instance {
				for (record_id, record) in model {
					for op in record {
						ops.push(CRDTOperation {
							instance: instance_id,
							model: model_str.clone(),
							record_id: record_id.clone(),
							timestamp: op.timestamp,
							id: op.id,
							data: op.data,
						})
					}
				}
			}
		}

		ops
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct CompressedCRDTOperation {
	pub timestamp: NTP64,
	pub id: Uuid,
	pub data: CRDTOperationData,
}

impl From<CRDTOperation> for CompressedCRDTOperation {
	fn from(value: CRDTOperation) -> Self {
		Self {
			timestamp: value.timestamp,
			id: value.id,
			data: value.data,
		}
	}
}
