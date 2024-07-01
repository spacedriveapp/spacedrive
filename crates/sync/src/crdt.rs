use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use specta::Type;
use uhlc::NTP64;
use uuid::Uuid;

pub enum OperationKind<'a> {
	Create,
	Update(&'a str),
	Delete,
}

impl fmt::Display for OperationKind<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			OperationKind::Create => write!(f, "c"),
			OperationKind::Update(field) => write!(f, "u:{field}"),
			OperationKind::Delete => write!(f, "d"),
		}
	}
}

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug, Type)]
pub enum CRDTOperationData {
	#[serde(rename = "c")]
	Create(#[specta(type = BTreeMap<String, serde_json::Value>)] BTreeMap<String, rmpv::Value>),
	#[serde(rename = "u")]
	Update {
		field: String,
		#[specta(type = serde_json::Value)]
		value: rmpv::Value,
	},
	#[serde(rename = "d")]
	Delete,
}

impl CRDTOperationData {
	#[must_use]
	pub fn create() -> Self {
		Self::Create(BTreeMap::default())
	}

	#[must_use]
	pub fn as_kind(&self) -> OperationKind<'_> {
		match self {
			Self::Create(_) => OperationKind::Create,
			Self::Update { field, .. } => OperationKind::Update(field),
			Self::Delete => OperationKind::Delete,
		}
	}
}

#[derive(PartialEq, Serialize, Deserialize, Clone, Type)]
pub struct CRDTOperation {
	pub instance: Uuid,
	#[specta(type = u32)]
	pub timestamp: NTP64,
	pub model: u16,
	#[specta(type = serde_json::Value)]
	pub record_id: rmpv::Value,
	pub data: CRDTOperationData,
}

impl CRDTOperation {
	#[must_use]
	pub fn kind(&self) -> OperationKind<'_> {
		self.data.as_kind()
	}
}

impl fmt::Debug for CRDTOperation {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("CRDTOperation")
			.field("data", &self.data)
			.field("model", &self.model)
			.field("record_id", &self.record_id.to_string())
			.finish_non_exhaustive()
	}
}
