use crate::{DevicePubId, ModelId};

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use uhlc::NTP64;

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

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub enum CRDTOperationData {
	#[serde(rename = "c")]
	Create(BTreeMap<String, rmpv::Value>),
	#[serde(rename = "u")]
	Update { field: String, value: rmpv::Value },
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

#[derive(PartialEq, Serialize, Deserialize, Clone)]
pub struct CRDTOperation {
	pub device_pub_id: DevicePubId,
	pub timestamp: NTP64,
	pub model_id: ModelId,
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
			.field("model", &self.model_id)
			.field("record_id", &self.record_id.to_string())
			.finish_non_exhaustive()
	}
}
