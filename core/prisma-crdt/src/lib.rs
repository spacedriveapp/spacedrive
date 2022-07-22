pub mod generator;
pub mod local;
pub mod owned;
pub mod relation;
pub mod shared;

mod attribute;
mod datamodel;
mod prelude;

use serde::{Deserialize, Serialize};
use serde_json::{Value, Map};
use uhlc::NTP64;

pub use owned::*;
pub use relation::*;
pub use shared::*;

pub type Id = Vec<u8>;

/// An operation on a CRDT - either a shared record or a many relation.
/// All CRDT operations record the `node` and `timestamp` the associated with them.
///
/// The state of a CRDT that an operation acts on is just the result of all previous operations,
/// so `CRDTOperation` is designed to be sent via any transport to any node that can resolve
/// that state.
#[derive(Serialize, Deserialize, Clone)]
pub struct CRDTOperation {
	#[serde(rename = "n")]
	pub node: Id,
	#[serde(rename = "t")]
	pub timestamp: NTP64, // HLC
	#[serde(flatten)]
	pub typ: CRDTOperationType,
}

impl CRDTOperation {
	pub fn new(node: Id, timestamp: NTP64, typ: CRDTOperationType) -> Self {
		Self {
			node,
			timestamp,
			typ,
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum CRDTOperationType {
	Shared(SharedOperation),
	Relation(RelationOperation),
	Owned(OwnedOperation),
}

impl CRDTOperationType {
	pub fn shared(model: &str, record_id: Value, data: SharedOperationData) -> Self {
		Self::Shared(SharedOperation {
			model: model.to_string(),
			record_id,
			data,
		})
	}

	pub fn relation(
		relation: &str,
		relation_item: Id,
		relation_group: Id,
		data: RelationOperationData,
	) -> Self {
		Self::Relation(RelationOperation::new(
			relation.to_string(),
			relation_item,
			relation_group,
			data,
		))
	}

	pub fn owned(model: &str, data: Vec<OwnedOperationData>) -> Self {
		Self::Owned(OwnedOperation {
			model: model.to_string(),
			data,
		})
	}
}

pub struct CRDTStore<Database> {
	pub database: Database,
}

pub type SerializedField = (String, Value);

pub fn objectify(val: impl Serialize) -> Map<String, Value> {
    match ::serde_json::to_value(val).unwrap() {
        serde_json::Value::Object(m) => m,
        _ => unreachable!(),
    }
}
