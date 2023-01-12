use std::{collections::BTreeMap, fmt::Debug};

use rspc::Type;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uhlc::NTP64;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub enum RelationOperationData {
	Create,
	Update { field: String, value: Value },
	Delete,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub struct RelationOperation {
	pub relation_item: Uuid,
	pub relation_group: Uuid,
	pub relation: String,
	pub data: RelationOperationData,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub enum SharedOperationCreateData {
	#[serde(rename = "u")]
	Unique(Map<String, Value>),
	#[serde(rename = "a")]
	Atomic,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(untagged)]
pub enum SharedOperationData {
	Create(SharedOperationCreateData),
	Update { field: String, value: Value },
	Delete,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub struct SharedOperation {
	pub record_id: Value,
	pub model: String,
	pub data: SharedOperationData,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub enum OwnedOperationData {
	Create(Value, BTreeMap<String, Value>),
	CreateMany {
		values: Vec<(Value, BTreeMap<String, Value>)>,
		skip_duplicates: bool,
	},
	Update(Value, BTreeMap<String, Value>),
	Delete(Value),
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub struct OwnedOperation {
	pub model: String,
	pub data: Vec<OwnedOperationData>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(untagged)]
pub enum CRDTOperationType {
	Shared(SharedOperation),
	Relation(RelationOperation),
	Owned(OwnedOperation),
}

#[derive(Serialize, Deserialize, Clone, Type)]
pub struct CRDTOperation {
	pub node: Uuid,
	pub timestamp: NTP64,
	pub id: Uuid,
	// #[serde(flatten)]
	pub typ: CRDTOperationType,
}

impl Debug for CRDTOperation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CRDTOperation")
			.field("node", &self.node.to_string())
			.field("timestamp", &self.timestamp.to_string())
			.field("typ", &self.typ)
			.finish()
	}
}
