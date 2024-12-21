use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Represents a single notification.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Notification {
	#[serde(flatten)]
	pub id: NotificationId,
	pub data: NotificationData,
	pub read: bool,
	pub expires: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(tag = "type", content = "id", rename_all = "camelCase")]
pub enum NotificationId {
	Library(Uuid, u32),
	Node(u32),
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NotificationKind {
	Info,
	Success,
	Error,
	Warning,
}

/// Represents the data of a single notification.
/// This data is used by the frontend to properly display the notification.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct NotificationData {
	pub title: String,
	pub content: String,
	pub kind: NotificationKind,
}
