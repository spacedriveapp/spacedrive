use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use uhlc::NTP64;
use uuid::Uuid;

pub enum OperationKind<'a> {
	Create,
	Update(&'a str),
	Delete,
}

impl std::fmt::Display for OperationKind<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			OperationKind::Create => write!(f, "c"),
			OperationKind::Update(field) => write!(f, "u:{field}"),
			OperationKind::Delete => write!(f, "d"),
		}
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, Type)]
pub struct RelationOperation {
	pub relation_item: Value,
	pub relation_group: Value,
	pub relation: String,
	pub data: RelationOperationData,
}

impl RelationOperation {
	#[must_use] pub fn kind(&self) -> OperationKind {
		self.data.as_kind()
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, Type)]
pub enum RelationOperationData {
	#[serde(rename = "c")]
	Create,
	#[serde(rename = "u")]
	Update { field: String, value: Value },
	#[serde(rename = "d")]
	Delete,
}

impl RelationOperationData {
	fn as_kind(&self) -> OperationKind {
		match self {
			Self::Create => OperationKind::Create,
			Self::Update { field, .. } => OperationKind::Update(field),
			Self::Delete => OperationKind::Delete,
		}
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, Type)]
pub struct SharedOperation {
	pub record_id: Value,
	pub model: String,
	pub data: SharedOperationData,
}

impl SharedOperation {
	#[must_use] pub fn kind(&self) -> OperationKind {
		self.data.as_kind()
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, Type)]
pub enum SharedOperationData {
	#[serde(rename = "c")]
	Create,
	#[serde(rename = "u")]
	Update { field: String, value: Value },
	#[serde(rename = "d")]
	Delete,
}

impl SharedOperationData {
	fn as_kind(&self) -> OperationKind {
		match self {
			Self::Create => OperationKind::Create,
			Self::Update { field, .. } => OperationKind::Update(field),
			Self::Delete => OperationKind::Delete,
		}
	}
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

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug, Type)]
#[serde(untagged)]
pub enum CRDTOperationType {
	Shared(SharedOperation),
	Relation(RelationOperation),
	// Owned(OwnedOperation),
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Type)]
pub struct CRDTOperation {
	pub instance: Uuid,
	#[specta(type = u32)]
	pub timestamp: NTP64,
	pub id: Uuid,
	// #[serde(flatten)]
	pub typ: CRDTOperationType,
}

impl Debug for CRDTOperation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CRDTOperation")
			.field("instance", &self.instance.to_string())
			.field("timestamp", &self.timestamp.to_string())
			.field("typ", &self.typ)
			.finish()
	}
}
