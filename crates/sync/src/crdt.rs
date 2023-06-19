use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use specta::Type;
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
pub enum SharedOperationData {
	#[serde(rename = "c")]
	Create(Map<String, Value>),
	#[serde(rename = "u")]
	Update { field: String, value: Value },
	#[serde(rename = "d")]
	Delete,
}

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
pub struct SharedOperation {
	pub record_id: Value,
	pub model: String,
	pub data: SharedOperationData,
}

// #[derive(Serialize, Deserialize, Clone, Debug, Type)]
// pub enum OwnedOperationData {
// 	Create(BTreeMap<String, Value>),
// 	CreateMany {
// 		values: Vec<(Value, BTreeMap<String, Value>)>,
// 		skip_duplicates: bool,
// 	},
// 	Update(BTreeMap<String, Value>),
// 	Delete,
// }

// #[derive(Serialize, Deserialize, Clone, Debug, Type)]
// pub struct OwnedOperationItem {
// 	pub id: Value,
// 	pub data: OwnedOperationData,
// }

// #[derive(Serialize, Deserialize, Clone, Debug, Type)]
// pub struct OwnedOperation {
// 	pub model: String,
// 	pub items: Vec<OwnedOperationItem>,
// }

#[derive(Serialize, Deserialize, Clone, Debug, Type)]
#[serde(untagged)]
pub enum CRDTOperationType {
	Shared(SharedOperation),
	Relation(RelationOperation),
	// Owned(OwnedOperation),
}

#[derive(Serialize, Deserialize, Clone, Type)]
pub struct CRDTOperation {
	pub node: Uuid,
	#[specta(type = u32)]
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
