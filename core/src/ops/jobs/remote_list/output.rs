use crate::service::network::RemoteJobState;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteJobsForDeviceOutput {
	pub jobs: Vec<RemoteJobState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RemoteJobsAllDevicesOutput {
	pub jobs_by_device: HashMap<Uuid, Vec<RemoteJobState>>,
}
