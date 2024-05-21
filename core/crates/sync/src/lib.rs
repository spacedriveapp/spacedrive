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

#[derive(Clone, Debug)]
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
	pub active: AtomicBool,
	pub active_notify: tokio::sync::Notify,
	pub actors: Arc<sd_actors::Actors>,
}

#[must_use]
pub fn crdt_op_db(op: &CRDTOperation) -> crdt_operation::Create {
	crdt_operation::Create {
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data).unwrap(),
		model: op.model as i32,
		record_id: rmp_serde::to_vec(&op.record_id).unwrap(),
		_params: vec![],
	}
}

#[must_use]
pub fn crdt_op_unchecked_db(
	op: &CRDTOperation,
	instance_id: i32,
) -> crdt_operation::CreateUnchecked {
	crdt_operation::CreateUnchecked {
		timestamp: op.timestamp.0 as i64,
		instance_id,
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data).unwrap(),
		model: op.model as i32,
		record_id: rmp_serde::to_vec(&op.record_id).unwrap(),
		_params: vec![],
	}
}
