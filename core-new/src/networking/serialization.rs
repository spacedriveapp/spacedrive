//! Common serialization utilities for networking module

use serde::{Serialize, de::DeserializeOwned};
use crate::networking::{NetworkError, Result};

/// Serialize a message to JSON bytes
pub fn serialize_message<T: Serialize>(message: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(message)
        .map_err(|e| NetworkError::SerializationError(format!("Serialization failed: {}", e)))
}

/// Deserialize a message from JSON bytes
pub fn deserialize_message<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
    serde_json::from_slice(data)
        .map_err(|e| NetworkError::SerializationError(format!("Deserialization failed: {}", e)))
}

/// Serialize a message with specific error context
pub fn serialize_with_context<T: Serialize>(message: &T, context: &str) -> Result<Vec<u8>> {
    serde_json::to_vec(message)
        .map_err(|e| NetworkError::SerializationError(format!("{}: {}", context, e)))
}

/// Deserialize a message with specific error context
pub fn deserialize_with_context<T: DeserializeOwned>(data: &[u8], context: &str) -> Result<T> {
    serde_json::from_slice(data)
        .map_err(|e| NetworkError::SerializationError(format!("{}: {}", context, e)))
}