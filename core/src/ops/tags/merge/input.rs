//! Input for merging delivered assertions, and the wire shapes an outbox
//! payload carries. The rows are final data authored elsewhere; delivery is
//! a set union on the owner, so replaying a batch is safe.

use sd_store::{Stamp, TagAssertion, TagDefinition};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WireDefinition {
	pub uuid: Uuid,
	pub slug_id: Uuid,
	pub path: String,
	pub color: Option<String>,
	pub icon: Option<String>,
	pub updated_hlc: String,
	pub origin_device: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WireAssertion {
	pub tag_uuid: Uuid,
	pub record_uuid: Uuid,
	pub external_id: Option<String>,
	pub content_uuid: Option<Uuid>,
	pub asserted: bool,
	pub hlc: String,
	pub device_uuid: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct MergeAssertionsInput {
	/// A source this device owns; the merge refuses anything else.
	pub source_uuid: Uuid,
	/// Every definition the assertions reference, so the store can always
	/// name the tags it carries.
	pub definitions: Vec<WireDefinition>,
	pub assertions: Vec<WireAssertion>,
}

impl MergeAssertionsInput {
	pub fn validate(&self) -> Result<(), String> {
		if self.source_uuid.is_nil() {
			return Err("source_uuid cannot be nil".to_string());
		}
		if self.assertions.is_empty() {
			return Err("assertions cannot be empty".to_string());
		}
		if self.assertions.len() > 10_000 {
			return Err("cannot merge more than 10000 assertions at once".to_string());
		}
		Ok(())
	}
}

impl From<&TagDefinition> for WireDefinition {
	fn from(definition: &TagDefinition) -> Self {
		Self {
			uuid: definition.uuid,
			slug_id: definition.slug_id,
			path: definition.path.clone(),
			color: definition.color.clone(),
			icon: definition.icon.clone(),
			updated_hlc: definition.updated_hlc.clone(),
			origin_device: definition.origin_device,
		}
	}
}

impl From<&WireDefinition> for TagDefinition {
	fn from(wire: &WireDefinition) -> Self {
		Self {
			uuid: wire.uuid,
			slug_id: wire.slug_id,
			path: wire.path.clone(),
			color: wire.color.clone(),
			icon: wire.icon.clone(),
			updated_hlc: wire.updated_hlc.clone(),
			origin_device: wire.origin_device,
		}
	}
}

impl From<&TagAssertion> for WireAssertion {
	fn from(assertion: &TagAssertion) -> Self {
		Self {
			tag_uuid: assertion.tag_uuid,
			record_uuid: assertion.record_uuid,
			external_id: assertion.external_id.clone(),
			content_uuid: assertion.content_uuid,
			asserted: assertion.asserted,
			hlc: assertion.stamp.hlc.clone(),
			device_uuid: assertion.stamp.device_uuid,
		}
	}
}

impl From<&WireAssertion> for TagAssertion {
	fn from(wire: &WireAssertion) -> Self {
		Self {
			tag_uuid: wire.tag_uuid,
			record_uuid: wire.record_uuid,
			external_id: wire.external_id.clone(),
			content_uuid: wire.content_uuid,
			asserted: wire.asserted,
			stamp: Stamp {
				hlc: wire.hlc.clone(),
				device_uuid: wire.device_uuid,
			},
		}
	}
}
