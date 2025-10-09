//! VDFS operations
//!
//! Create, update, and query entries in the Virtual Distributed File System.

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::Arc;
use uuid::Uuid;

use crate::ffi::WireClient;
use crate::types::{EntryType, Result};

/// VDFS client for file system operations
pub struct VdfsClient {
	client: Arc<RefCell<WireClient>>,
}

impl VdfsClient {
	pub(crate) fn new(client: Arc<RefCell<WireClient>>) -> Self {
		Self { client }
	}

	/// Create a new entry in VDFS
	pub fn create_entry(&self, input: CreateEntry) -> Result<Entry> {
		self.client
			.borrow()
			.call("action:vdfs.create_entry.input.v1", &input)
	}

	/// Update entry metadata
	pub fn update_metadata(&self, entry_id: Uuid, metadata: serde_json::Value) -> Result<()> {
		self.client.borrow().call(
			"action:vdfs.update_metadata.input.v1",
			&UpdateMetadata { entry_id, metadata },
		)
	}

	/// Write sidecar file
	pub fn write_sidecar(&self, entry_id: Uuid, filename: &str, data: &[u8]) -> Result<()> {
		self.client.borrow().call(
			"action:vdfs.write_sidecar.input.v1",
			&WriteSidecar {
				entry_id,
				filename: filename.to_string(),
				data: BASE64_STANDARD.encode(data),
			},
		)
	}

	/// Read sidecar file
	pub fn read_sidecar(&self, entry_id: Uuid, filename: &str) -> Result<Vec<u8>> {
		let result: ReadSidecarOutput = self.client.borrow().call(
			"query:vdfs.read_sidecar.v1",
			&ReadSidecar {
				entry_id,
				filename: filename.to_string(),
			},
		)?;

		BASE64_STANDARD
			.decode(&result.data)
			.map_err(|e| crate::types::Error::InvalidInput(e.to_string()))
	}

	/// List entries in a location
	pub fn list_entries(&self, location_id: Uuid) -> Result<Vec<Entry>> {
		self.client
			.borrow()
			.call("query:vdfs.list_entries.v1", &ListEntries { location_id })
	}
}

// === Input/Output Types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntry {
	pub name: String,
	pub path: String,
	#[serde(rename = "entry_type")]
	pub entry_type: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
	pub id: Uuid,
	pub name: String,
	pub path: String,
	pub entry_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateMetadata {
	entry_id: Uuid,
	metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct WriteSidecar {
	entry_id: Uuid,
	filename: String,
	data: String, // base64-encoded
}

#[derive(Debug, Serialize, Deserialize)]
struct ReadSidecar {
	entry_id: Uuid,
	filename: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReadSidecarOutput {
	data: String, // base64-encoded
}

#[derive(Debug, Serialize, Deserialize)]
struct ListEntries {
	location_id: Uuid,
}
