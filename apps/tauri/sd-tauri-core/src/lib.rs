// sd-tauri-core: FFI bridge between Tauri and Spacedrive Core
// This crate provides the interface layer for embedding the core in a Tauri application

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
	pub jsonrpc: String,
	pub method: String,
	pub params: serde_json::Value,
	pub id: String,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
	pub jsonrpc: String,
	pub id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
	pub code: i32,
	pub message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<serde_json::Value>,
}

// Core state management will be added here once we understand
// the core's initialization patterns better
// For now this is a skeleton that provides the types

pub mod commands {
	// Tauri command implementations will go here
	// Following the pattern from sd-ios-core but for Tauri's IPC
}

/// Platform-specific data directory resolution
pub fn default_data_dir() -> anyhow::Result<std::path::PathBuf> {
	#[cfg(target_os = "macos")]
	let dir = dirs::data_dir()
		.ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
		.join("spacedrive");

	#[cfg(target_os = "windows")]
	let dir = dirs::data_dir()
		.ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
		.join("Spacedrive");

	#[cfg(target_os = "linux")]
	let dir = dirs::data_local_dir()
		.ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
		.join("spacedrive");

	// Create directory if it doesn't exist
	std::fs::create_dir_all(&dir)?;

	Ok(dir)
}
