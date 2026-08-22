//! The process resource: one supervised service as an event-emitting,
//! frontend-cacheable resource.

use crate::domain::resource::Identifiable;
use sd_supervisor::ProcessStatus;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Namespace for deriving a stable resource id from a service name — services
/// are in-memory rows keyed by name, and the id must survive restarts.
const PROCESS_NAMESPACE: Uuid = Uuid::from_bytes([
	0x8f, 0x2b, 0x1c, 0x5e, 0x74, 0x3a, 0x4d, 0x91, 0xb6, 0x0d, 0xe4, 0x52, 0x7a, 0x19, 0xc8, 0x03,
]);

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Process {
	pub id: Uuid,
	#[serde(flatten)]
	pub status: ProcessStatus,
}

impl From<ProcessStatus> for Process {
	fn from(status: ProcessStatus) -> Self {
		Self {
			id: Uuid::new_v5(&PROCESS_NAMESPACE, status.name.as_bytes()),
			status,
		}
	}
}

impl Identifiable for Process {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str
	where
		Self: Sized,
	{
		"process"
	}
}

crate::register_resource!(Process);
