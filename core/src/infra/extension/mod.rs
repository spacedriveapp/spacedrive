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

mod host_functions;
mod manager;
mod permissions;
mod types;

pub use manager::PluginManager;
pub use permissions::{ExtensionPermissions, PermissionError};
pub use types::{ExtensionManifest, PluginManifest};
