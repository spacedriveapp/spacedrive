use serde::{Deserialize, Serialize};

/// Action/Query envelopes for client-agnostic RPC
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonRequest {
	Ping,
	Action { method: String, payload: Vec<u8> },
	Query { method: String, payload: Vec<u8> },
	Shutdown,
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
	Error(DaemonError),
}
