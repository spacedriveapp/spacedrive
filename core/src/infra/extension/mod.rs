//! WASM Plugin System
//!
//! This module provides a secure WebAssembly-based extension system for Spacedrive.
//! Extensions are sandboxed WASM modules that can extend Spacedrive's functionality
//! while maintaining security and stability.
//!
//! ## Architecture
//!
//! Extensions communicate with Spacedrive Core via a minimal host function API.
//! The key insight: we expose ONE generic `spacedrive_call()` function that routes
//! to the existing Wire operation registry, reusing all daemon RPC infrastructure.
//!
//! ## Components
//!
//! - `manager`: Plugin lifecycle management (load, unload, hot-reload)
//! - `host_functions`: WASM host functions (bridge to operation registry)
//! - `permissions`: Capability-based security model
//! - `types`: Shared types and manifest format

#[cfg(feature = "wasm")]
mod host_functions;
#[cfg(feature = "wasm")]
mod job_registry;
#[cfg(feature = "wasm")]
mod manager;
#[cfg(feature = "wasm")]
mod permissions;
#[cfg(feature = "wasm")]
mod types;
#[cfg(feature = "wasm")]
mod wasm_job;

#[cfg(feature = "wasm")]
pub use job_registry::{ExtensionJobRegistration, ExtensionJobRegistry};
#[cfg(feature = "wasm")]
pub use manager::PluginManager;
#[cfg(feature = "wasm")]
pub use permissions::{ExtensionPermissions, PermissionError};
#[cfg(feature = "wasm")]
pub use types::{ExtensionManifest, PluginManifest};
#[cfg(feature = "wasm")]
pub use wasm_job::WasmJob;
