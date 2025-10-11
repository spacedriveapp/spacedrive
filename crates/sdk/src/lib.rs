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

#![allow(async_fn_in_trait)]

pub mod actions;
pub mod agent;
pub mod ai;
pub mod ffi;
pub mod job_context;
pub mod models;
pub mod query;
pub mod tasks;
pub mod types;
pub mod vdfs;

// Re-export for convenience
pub use actions::*;
pub use agent::{
	AgentContext, AgentMemory, AssociativeMemory, AssociativeQuery, JobDispatchBuilder,
	JobDispatcher, MemoryHandle, MemoryReadGuard, MemoryVariant, MemoryWriteGuard,
	NotificationBuilder, TemporalMemory, TemporalQuery, WorkingMemory,
};
pub use ai::*;
pub use job_context::JobContext as SdkJobContext;
pub use models::*;
pub use query::*;
pub use tasks::*;
pub use types::*;
pub use vdfs::*;

/// Prelude with commonly used types
pub mod prelude {
	pub use crate::actions::*;
	pub use crate::agent::*;
	pub use crate::ai::*;
	pub use crate::job_context::{JobContext, JobResult};
	pub use crate::models::*;
	pub use crate::query::*;
	pub use crate::tasks::*;
	pub use crate::types::*;
	pub use crate::vdfs::*;
	pub use serde::{Deserialize, Serialize};
}

// Re-export macros
pub use spacedrive_sdk_macros::{
	action, action_execute, agent, agent_memory, agent_trail, extension, filter, job,
	memory_config, model, on_event, on_startup, persist_strategy, query, scheduled, setting, task,
};
