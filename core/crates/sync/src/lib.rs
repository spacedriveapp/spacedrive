#![allow(clippy::unwrap_used, clippy::panic)] // TODO: Brendan remove this once you've got error handling here

mod actor;
mod db_operation;
pub mod ingest;
mod manager;

use sd_prisma::prisma::*;
use sd_sync::*;

use std::{collections::HashMap, sync::Arc};

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
	pub instance: uuid::Uuid,
	pub timestamps: Timestamps,
	pub clock: uhlc::HLC,
}

pub fn shared_op_db(op: &CRDTOperation, shared_op: &SharedOperation) -> shared_operation::Create {
	shared_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: shared_op.kind().to_string(),
		data: serde_json::to_vec(&shared_op.data).unwrap(),
		model: shared_op.model.to_string(),
		record_id: serde_json::to_vec(&shared_op.record_id).unwrap(),
		_params: vec![],
	}
}

pub fn relation_op_db(
	op: &CRDTOperation,
	relation_op: &RelationOperation,
) -> relation_operation::Create {
	relation_operation::Create {
		id: op.id.as_bytes().to_vec(),
		timestamp: op.timestamp.0 as i64,
		instance: instance::pub_id::equals(op.instance.as_bytes().to_vec()),
		kind: relation_op.kind().to_string(),
		data: serde_json::to_vec(&relation_op.data).unwrap(),
		relation: relation_op.relation.to_string(),
		item_id: serde_json::to_vec(&relation_op.relation_item).unwrap(),
		group_id: serde_json::to_vec(&relation_op.relation_group).unwrap(),
		_params: vec![],
	}
}
