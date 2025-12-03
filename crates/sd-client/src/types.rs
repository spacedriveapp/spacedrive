use serde::Serialize;

// Re-export types from sd-core
pub use sd_core::domain::addressing::SdPath;
pub use sd_core::domain::content_identity::ContentIdentity;
pub use sd_core::domain::file::{File, Sidecar};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct QueryRequest {
	pub method: String,
	pub library_id: Option<String>,
	pub payload: serde_json::Value,
}
