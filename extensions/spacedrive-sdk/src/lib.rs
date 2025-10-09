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

pub mod ffi;
pub mod job_context;
pub mod types;

pub use job_context::JobContext as SdkJobContext;
pub use types::*;

/// Prelude with commonly used types
pub mod prelude {
	pub use crate::job_context::{JobContext, JobResult};
	pub use crate::types::{Error, Result};
	pub use serde::{Deserialize, Serialize};
	pub use uuid::Uuid;
}

// Re-export macros
pub use spacedrive_sdk_macros::{extension, spacedrive_job};
