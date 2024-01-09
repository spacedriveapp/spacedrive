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
pub enum CRDTOperationData {
	#[serde(rename = "c")]
	Create,
	#[serde(rename = "u")]
	Update { field: String, value: Value },
	#[serde(rename = "d")]
	Delete,
}

impl CRDTOperationData {
	pub fn as_kind(&self) -> OperationKind {
		match self {
			Self::Create => OperationKind::Create,
			Self::Update { field, .. } => OperationKind::Update(field),
			Self::Delete => OperationKind::Delete,
		}
	}
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Type)]
pub struct CRDTOperation {
	pub instance: Uuid,
	#[specta(type = u32)]
	pub timestamp: NTP64,
	pub id: Uuid,
	pub model: String,
	pub record_id: Value,
	pub data: CRDTOperationData,
}

impl CRDTOperation {
	#[must_use]
	pub fn kind(&self) -> OperationKind {
		self.data.as_kind()
	}
}

impl Debug for CRDTOperation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CRDTOperation")
			.field("instance", &self.instance.to_string())
			.field("timestamp", &self.timestamp.to_string())
			// .field("typ", &self.typ)
			.finish()
	}
}
