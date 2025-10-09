//! Spacedrive Extension SDK
//!
//! Beautiful, type-safe API for building Spacedrive WASM extensions.
//!
//! # Example
//!
//! ```no_run
//! use spacedrive_sdk::{ExtensionContext, prelude::*};
//!
//! #[spacedrive_extension]
//! fn init(ctx: &mut ExtensionContext) -> Result<()> {
//!     ctx.log("Finance extension starting...");
//!
//!     // Create entry
//!     let entry = ctx.vdfs().create_entry(CreateEntry {
//!         name: "Receipt: Starbucks".into(),
//!         path: "receipts/1.eml".into(),
//!         entry_type: "FinancialDocument".into(),
//!     })?;
//!
//!     // Run OCR
//!     let ocr_result = ctx.ai().ocr(&pdf_data, OcrOptions::default())?;
//!
//!     // Store sidecar
//!     ctx.vdfs().write_sidecar(entry.id, "ocr.txt", ocr_result.text.as_bytes())?;
//!
//!     Ok(())
//! }
//! ```

pub mod ai;
pub mod credentials;
pub mod ffi;
pub mod job_context;
pub mod jobs;
pub mod types;
pub mod vdfs;

pub use job_context::JobContext as SdkJobContext;
pub use types::*;

/// Prelude with commonly used types
pub mod prelude {
	pub use crate::ai::{OcrOptions, OcrResult};
	pub use crate::job_context::{JobContext, JobResult};
	pub use crate::types::{Error, Result};
	pub use crate::vdfs::{CreateEntry, Entry};
	pub use crate::ExtensionContext;
	pub use serde::{Deserialize, Serialize};
	pub use uuid::Uuid;
}

use std::cell::RefCell;
use std::sync::Arc;
use uuid::Uuid;

/// Main context for extension operations
///
/// This is the primary API surface for extensions. It provides access to all
/// Spacedrive capabilities in a type-safe, ergonomic way.
pub struct ExtensionContext {
	library_id: Uuid,
	client: Arc<RefCell<ffi::WireClient>>,
}

impl ExtensionContext {
	/// Create new extension context
	pub fn new(library_id: Uuid) -> Self {
		Self {
			library_id,
			client: Arc::new(RefCell::new(ffi::WireClient::new(library_id))),
		}
	}

	/// Get library ID
	pub fn library_id(&self) -> Uuid {
		self.library_id
	}

	/// VDFS operations
	pub fn vdfs(&self) -> vdfs::VdfsClient {
		vdfs::VdfsClient::new(self.client.clone())
	}

	/// AI operations
	pub fn ai(&self) -> ai::AiClient {
		ai::AiClient::new(self.client.clone())
	}

	/// Credential operations
	pub fn credentials(&self) -> credentials::CredentialClient {
		credentials::CredentialClient::new(self.client.clone())
	}

	/// Job operations
	pub fn jobs(&self) -> jobs::JobClient {
		jobs::JobClient::new(self.client.clone())
	}

	/// Log a message
	pub fn log(&self, message: &str) {
		ffi::log_info(message);
	}

	/// Log an error
	pub fn log_error(&self, message: &str) {
		ffi::log_error(message);
	}
}

// Re-export macros
pub use spacedrive_sdk_macros::{extension, spacedrive_job};
