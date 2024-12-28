#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use sd_prisma::prisma::{cloud_crdt_operation, crdt_operation};
use sd_utils::uuid_to_bytes;

use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;

use sd_core_shared_errors::library_sync::Error;

pub mod backfill;
mod db_operation;
mod ingest_utils;
mod manager;

pub use db_operation::{from_cloud_crdt_ops, from_crdt_ops, write_crdt_op_to_db};
pub use manager::Manager as SyncManager;
pub use uhlc::NTP64;

pub use sd_core_shared_types::db_types::DevicePubId;
pub use sd_sync::{
	CRDTOperation, CompressedCRDTOperation, CompressedCRDTOperationsPerModel,
	CompressedCRDTOperationsPerModelPerDevice, ModelId, OperationFactory, RecordId, RelationSyncId,
	RelationSyncModel, SharedSyncModel, SyncId, SyncModel,
};

pub type TimestampPerDevice = Arc<RwLock<HashMap<DevicePubId, NTP64>>>;

pub fn crdt_op_db(op: &CRDTOperation) -> Result<crdt_operation::Create, Error> {
	Ok(crdt_operation::Create {
		timestamp: {
			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we had to store using i64 due to SQLite limitations
			{
				op.timestamp.as_u64() as i64
			}
		},
		device_pub_id: uuid_to_bytes(&op.device_pub_id),
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data)?,
		model: i32::from(op.model_id),
		record_id: rmp_serde::to_vec(&op.record_id)?,
		_params: vec![],
	})
}

pub fn crdt_op_unchecked_db(op: &CRDTOperation) -> Result<crdt_operation::CreateUnchecked, Error> {
	Ok(crdt_operation::CreateUnchecked {
		timestamp: {
			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we had to store using i64 due to SQLite limitations
			{
				op.timestamp.as_u64() as i64
			}
		},
		device_pub_id: uuid_to_bytes(&op.device_pub_id),
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data)?,
		model: i32::from(op.model_id),
		record_id: rmp_serde::to_vec(&op.record_id)?,
		_params: vec![],
	})
}

pub fn cloud_crdt_op_db(op: &CRDTOperation) -> Result<cloud_crdt_operation::Create, Error> {
	Ok(cloud_crdt_operation::Create {
		timestamp: {
			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we had to store using i64 due to SQLite limitations
			{
				op.timestamp.as_u64() as i64
			}
		},
		device_pub_id: uuid_to_bytes(&op.device_pub_id),
		kind: op.data.as_kind().to_string(),
		data: rmp_serde::to_vec(&op.data)?,
		model: i32::from(op.model_id),
		record_id: rmp_serde::to_vec(&op.record_id)?,
		_params: vec![],
	})
}
