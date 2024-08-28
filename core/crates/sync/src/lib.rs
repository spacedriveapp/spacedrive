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

use sd_prisma::prisma::{crdt_operation, device, PrismaClient};
use sd_sync::{CRDTOperation, ModelId};

use std::{
	collections::HashMap,
	sync::{atomic::AtomicBool, Arc},
};

use tokio::sync::{Notify, RwLock};

mod actor;
pub mod backfill;
mod db_operation;
pub mod ingest;
mod manager;

pub use ingest::{Actor, Event, Handler, MessagesEvent, Request, State};
pub use manager::{GetOpsArgs, Manager as SyncManager};
pub use uhlc::NTP64;

#[derive(Clone, Debug)]
pub enum SyncEvent {
	Ingested,
	Created,
}

pub use sd_core_prisma_helpers::DevicePubId;

pub type TimestampPerDevice = Arc<RwLock<HashMap<DevicePubId, NTP64>>>;

pub struct SharedState {
	pub db: Arc<PrismaClient>,
	pub emit_messages_flag: Arc<AtomicBool>,
	pub device_pub_id: DevicePubId,
	pub timestamp_per_device: TimestampPerDevice,
	pub clock: uhlc::HLC,
	pub active: AtomicBool,
	pub active_notify: Notify,
	pub actors: Arc<sd_actors::Actors>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("serialization error: {0}")]
	Serialization(#[from] rmp_serde::encode::Error),
	#[error("deserialization error: {0}")]
	Deserialization(#[from] rmp_serde::decode::Error),
	#[error("database error: {0}")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("invalid model id: {0}")]
	InvalidModelId(ModelId),
	#[error("tried to write an empty operations list")]
	EmptyOperations,
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::Database(e) => e.into(),
			Error::InvalidModelId(id) => Self::new(
				rspc::ErrorCode::BadRequest,
				format!("Invalid model id <id={id}>"),
			),
			_ => Self::with_cause(
				rspc::ErrorCode::InternalServerError,
				"Internal sync error".to_string(),
				e,
			),
		}
	}
}

pub fn crdt_op_db(op: &CRDTOperation) -> Result<crdt_operation::Create, Error> {
	Ok(crdt_operation::Create {
		timestamp: {
			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we had to store using i64 due to SQLite limitations
			{
				op.timestamp.as_u64() as i64
			}
		},
		device: device::pub_id::equals(op.device_pub_id.as_bytes().to_vec()),
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data)?,
		model: i32::from(op.model_id),
		record_id: rmp_serde::to_vec(&op.record_id)?,
		_params: vec![],
	})
}

pub fn crdt_op_unchecked_db(
	op: &CRDTOperation,
	device_pub_id: &DevicePubId,
) -> Result<crdt_operation::CreateUnchecked, Error> {
	Ok(crdt_operation::CreateUnchecked {
		timestamp: {
			#[allow(clippy::cast_possible_wrap)]
			// SAFETY: we had to store using i64 due to SQLite limitations
			{
				op.timestamp.as_u64() as i64
			}
		},
		device_pub_id: device_pub_id.to_db(),
		kind: op.kind().to_string(),
		data: rmp_serde::to_vec(&op.data)?,
		model: i32::from(op.model_id),
		record_id: rmp_serde::to_vec(&op.record_id)?,
		_params: vec![],
	})
}
