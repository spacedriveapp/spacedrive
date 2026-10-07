//! Spacedrive Extension SDK
//!
//! Beautiful, type-safe API for building Spacedrive WASM extensions.
//!
//! # Example
//!
//! ```ignore
//! use spacedrive_sdk::prelude::*;
//!
//! #[job(name = "sizes")]
//! async fn sizes(ctx: &JobContext, state: &mut SizesState) -> Result<()> {
//!     let photos = ctx.vdfs().query_records().of_type::<Image>().collect().await?;
//!     for photo in photos {
//!         let bytes = photo.read().await?;
//!         ctx.log(&format!("{} is {} bytes", photo.name(), bytes.len()));
//!     }
//!     Ok(())
//! }
//! ```

#![allow(async_fn_in_trait, warnings)]

pub mod actions;
pub mod agent;
pub mod ai;
pub mod clock;
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
pub use clock::now;
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
	pub use crate::clock::now;
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
