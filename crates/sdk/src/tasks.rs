//! Task system for job composition
//!
//! Stubs for type-checking. Implementation will call WASM host functions.

use crate::types::*;
use crate::{ai::AiContext, vdfs::VdfsContext};

/// Task execution context
pub struct TaskContext;

impl TaskContext {
	/// Access VDFS
	pub fn vdfs(&self) -> VdfsContext {
		VdfsContext
	}

	/// Access AI
	pub fn ai(&self) -> AiContext {
		AiContext
	}

	/// Access extension config
	pub fn config<C>(&self) -> &C {
		panic!("Access config")
	}

	/// Read sidecar data
	pub async fn read_sidecar<T: serde::de::DeserializeOwned>(
		&self,
		content_uuid: Uuid,
		kind: &str,
	) -> Result<T> {
		panic!("WASM host call")
	}
}

/// Task result type
pub type TaskResult<T> = std::result::Result<T, Error>;
