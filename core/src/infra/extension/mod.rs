//! WASM Plugin System
//!
//! This module provides a secure WebAssembly-based extension system for Spacedrive.
//! Extensions are sandboxed WASM modules that can extend Spacedrive's functionality
//! while maintaining security and stability.
//!
//! ## Architecture
//!
//! Extensions communicate with Spacedrive Core through a small set of host
//! functions. Everything that returns data goes through one of them,
//! `spacedrive_op`, whose operations `ops` answers on the running job's async
//! side with the manifest's grants checked there.
//!
//! ## Components
//!
//! - `manager`: Plugin lifecycle management (load, unload, hot-reload)
//! - `host_functions`: WASM host functions
//! - `ops`: the operations behind `spacedrive_op`
//! - `types`: the manifest, its permission grants, and the loaded plugin

#[cfg(feature = "wasm")]
mod host_functions;
#[cfg(feature = "wasm")]
mod job_registry;
#[cfg(feature = "wasm")]
mod manager;
#[cfg(feature = "wasm")]
mod model_registry;
#[cfg(feature = "wasm")]
mod ops;
#[cfg(feature = "wasm")]
mod types;
#[cfg(feature = "wasm")]
mod wasm_job;

#[cfg(feature = "wasm")]
pub use job_registry::{ExtensionJobRegistration, ExtensionJobRegistry};
#[cfg(feature = "wasm")]
pub use manager::PluginManager;
#[cfg(feature = "wasm")]
pub use model_registry::{ExtensionModelRegistry, ModelDefinition};
#[cfg(feature = "wasm")]
pub use types::{ExtensionManifest, ManifestPermissions, PluginManifest};
#[cfg(feature = "wasm")]
pub use wasm_job::WasmJob;
