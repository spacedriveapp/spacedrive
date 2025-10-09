//! Types for the WASM plugin system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Extension manifest (manifest.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
	pub id: String,
	pub name: String,
	pub version: String,
	pub description: String,
	pub author: String,
	pub homepage: Option<String>,

	/// WASM file path (relative to manifest)
	pub wasm_file: PathBuf,

	/// Permissions required by this extension
	pub permissions: ManifestPermissions,

	/// Configuration schema (JSON Schema)
	#[serde(default)]
	pub config_schema: Option<serde_json::Value>,
}

/// Permission declaration in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestPermissions {
	/// Wire methods this extension can call (prefix matching)
	/// e.g., ["vdfs.", "ai.ocr", "credentials.store"]
	pub methods: Vec<String>,

	/// Libraries this extension can access
	/// "*" = all libraries, or specific UUIDs
	#[serde(default = "default_all_libraries")]
	pub libraries: Vec<String>,

	/// Rate limits
	#[serde(default)]
	pub rate_limits: RateLimits,

	/// Network access (for HTTP proxy)
	#[serde(default)]
	pub network_access: Vec<String>,

	/// Resource limits
	#[serde(default)]
	pub max_memory_mb: usize,
}

fn default_all_libraries() -> Vec<String> {
	vec!["*".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimits {
	#[serde(default = "default_requests_per_minute")]
	pub requests_per_minute: usize,

	#[serde(default = "default_concurrent_jobs")]
	pub concurrent_jobs: usize,
}

fn default_requests_per_minute() -> usize {
	1000
}

fn default_concurrent_jobs() -> usize {
	10
}

impl Default for RateLimits {
	fn default() -> Self {
		Self {
			requests_per_minute: 1000,
			concurrent_jobs: 10,
		}
	}
}

impl Default for ManifestPermissions {
	fn default() -> Self {
		Self {
			methods: vec![],
			libraries: vec!["*".to_string()],
			rate_limits: RateLimits::default(),
			network_access: vec![],
			max_memory_mb: 512,
		}
	}
}

/// Loaded plugin instance
#[derive(Debug)]
pub struct LoadedPlugin {
	pub id: String,
	pub manifest: ExtensionManifest,
	pub loaded_at: DateTime<Utc>,
}

/// Alias for consistency with other code
pub type PluginManifest = ExtensionManifest;
