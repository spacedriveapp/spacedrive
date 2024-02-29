#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Brendan remove this once you've got error handling here

mod actor;
pub mod backfill;
mod db_operation;
pub mod ingest;
mod manager;

use sd_prisma::prisma::{crdt_operation, instance, PrismaClient};
use sd_sync::CRDTOperation;

use std::{
	collections::HashMap,
	sync::{atomic::AtomicBool, Arc},
};

pub use ingest::*;
pub use manager::*;
pub use uhlc::NTP64;

#[derive(Clone)]
pub enum SyncMessage {
	Ingested,
	Created,
}

pub type Timestamps = Arc<tokio::sync::RwLock<HashMap<uuid::Uuid, NTP64>>>;

pub struct SharedState {
	pub db: Arc<PrismaClient>,
	pub emit_messages_flag: Arc<AtomicBool>,
	pub instance: uuid::Uuid,
	pub timestamps: Timestamps,
	pub clock: uhlc::HLC,
}

#[must_use]
pub fn crdt_op_db(op: &CRDTOperation) -> crdt_operation::Create {
	crdt_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: op.kind().to_string(),
		data: serde_json::to_vec(&op.data).unwrap(),
		model: op.model.to_string(),
		record_id: serde_json::to_vec(&op.record_id).unwrap(),
		_params: vec![],
	}
}

#[must_use]
pub fn crdt_op_unchecked_db(
	op: &CRDTOperation,
	instance_id: i32,
) -> crdt_operation::CreateUnchecked {
	crdt_operation::CreateUnchecked {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance_id,
		kind: op.kind().to_string(),
		data: serde_json::to_vec(&op.data).unwrap(),
		model: op.model.to_string(),
		record_id: serde_json::to_vec(&op.record_id).unwrap(),
		_params: vec![],
	}
}
