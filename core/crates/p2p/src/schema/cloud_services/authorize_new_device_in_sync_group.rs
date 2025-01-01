use sd_cloud_schema::{devices, libraries, sync::groups};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
	pub sync_group: groups::GroupWithDevices,
	pub asking_device: devices::Device,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Response {
	Accepted {
		authorizor_device: devices::Device,
		keys: Vec<Vec<u8>>,
		library_pub_id: libraries::PubId,
		library_name: String,
		library_description: Option<String>,
	},
	Rejected,
	TimedOut,
}
