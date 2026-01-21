use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::service::network::device::{DeviceInfo, SessionKeys};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VouchPayload {
	pub vouchee_device_id: Uuid,
	pub vouchee_public_key: Vec<u8>,
	pub vouchee_device_info: DeviceInfo,
	pub timestamp: DateTime<Utc>,
	pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptedDevice {
	pub device_info: DeviceInfo,
	pub session_keys: SessionKeys,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectedDevice {
	pub device_id: Uuid,
	pub device_name: String,
	pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VouchingSession {
	pub id: Uuid,
	pub vouchee_device_id: Uuid,
	pub vouchee_device_name: String,
	pub voucher_device_id: Uuid,
	pub created_at: DateTime<Utc>,
	pub state: VouchingSessionState,
	pub vouches: Vec<VouchState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum VouchingSessionState {
	Pending,
	InProgress,
	Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VouchState {
	pub device_id: Uuid,
	pub device_name: String,
	pub status: VouchStatus,
	pub updated_at: DateTime<Utc>,
	pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum VouchStatus {
	Selected,
	Queued,
	Waiting,
	Accepted,
	Rejected,
	Unreachable,
}
