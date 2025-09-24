use serde::{Deserialize, Serialize};

/// Action/Query envelopes for JSON-based RPC
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonRequest {
	Ping,
	Action {
		method: String,
		library_id: Option<uuid::Uuid>,
		payload: serde_json::Value,
	},
	Query {
		method: String,
		library_id: Option<uuid::Uuid>,
		payload: serde_json::Value,
	},
	/// Subscribe to real-time events
	Subscribe {
		/// Event types to subscribe to (empty = all events)
		event_types: Vec<String>,
		/// Optional filter for specific library/job/etc
		filter: Option<EventFilter>,
	},
	/// Unsubscribe from events
	Unsubscribe,
	Shutdown,
}

/// Filter criteria for event subscriptions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
	/// Filter by library ID
	pub library_id: Option<uuid::Uuid>,
	/// Filter by job ID
	pub job_id: Option<String>,
	/// Filter by device ID
	pub device_id: Option<uuid::Uuid>,
}

/// Comprehensive daemon error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonError {
	// Connection and I/O errors
	ConnectionFailed(String),
	ReadError(String),
	WriteError(String),

	// Request processing errors
	RequestTooLarge(String),
	InvalidRequest(String),
	SerializationError(String),
	DeserializationError(String),

	// Handler and operation errors
	HandlerNotFound(String),
	OperationFailed(String),
	CoreUnavailable(String),

	// Validation errors
	ValidationError(String),
	SecurityError(String),

	// Internal errors
	InternalError(String),
}

impl std::fmt::Display for DaemonError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			DaemonError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
			DaemonError::ReadError(msg) => write!(f, "Read error: {}", msg),
			DaemonError::WriteError(msg) => write!(f, "Write error: {}", msg),
			DaemonError::RequestTooLarge(msg) => write!(f, "Request too large: {}", msg),
			DaemonError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
			DaemonError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
			DaemonError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
			DaemonError::HandlerNotFound(method) => write!(f, "Handler not found: {}", method),
			DaemonError::OperationFailed(msg) => write!(f, "Operation failed: {}", msg),
			DaemonError::CoreUnavailable(msg) => write!(f, "Core unavailable: {}", msg),
			DaemonError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
			DaemonError::SecurityError(msg) => write!(f, "Security error: {}", msg),
			DaemonError::InternalError(msg) => write!(f, "Internal error: {}", msg),
		}
	}
}

impl std::error::Error for DaemonError {}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
	Pong,
	Ok(Vec<u8>),
	/// JSON response for external clients (converted from bincode)
	JsonOk(serde_json::Value),
	Error(DaemonError),
	/// Real-time event from the core event bus
	Event(crate::infra::event::Event),
	/// Subscription acknowledgment
	Subscribed,
	/// Unsubscription acknowledgment
	Unsubscribed,
}
