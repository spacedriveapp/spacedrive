use crate::schema::Service;

use nested_enum_utils::enum_conversions;
use serde::{Deserialize, Serialize};

pub mod authorize_new_device_in_sync_group;
pub mod notify_new_sync_messages;

#[allow(clippy::large_enum_variant)]
#[nested_enum_utils::enum_conversions(super::Request)]
#[derive(Debug, Serialize, Deserialize)]
#[quic_rpc_derive::rpc_requests(Service)]
pub enum Request {
	#[rpc(response = authorize_new_device_in_sync_group::Response)]
	AuthorizeNewDeviceInSyncGroup(authorize_new_device_in_sync_group::Request),
	#[rpc(response = notify_new_sync_messages::Response)]
	NotifyNewSyncMessages(notify_new_sync_messages::Request),
}

#[derive(Debug, Serialize, Deserialize)]
#[enum_conversions(super::Response)]
pub enum Response {
	AuthorizeNewDeviceInSyncGroup(authorize_new_device_in_sync_group::Response),
	NotifyNewSyncMessages(notify_new_sync_messages::Response),
}
