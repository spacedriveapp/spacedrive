//! Unified API layer for Spacedrive operations
//!
//! This module provides a clean, unified entry point for all operations
//! with built-in authentication, authorization, and session management.
//!
//! ## Architecture
//!
//! ```
//! Applications → ApiDispatcher → PermissionLayer → Operations
//! ```
//!
//! ## Key Components
//!
//! - **`ApiDispatcher`**: Main entry point for all operations
//! - **`SessionContext`**: Rich session context with auth/permissions
//! - **`PermissionLayer`**: Authentication and authorization
//! - **`ApiError`**: Unified error handling for API operations

pub mod context;
pub mod dispatcher;
pub mod error;
pub mod middleware;
pub mod permissions;
pub mod session;
pub mod types;

// Re-export main types for easy access
pub use context::RequestMetadata;
pub use dispatcher::ApiDispatcher;
pub use error::{ApiError, ApiResult};
pub use permissions::{AuthLevel, PermissionError, PermissionLayer, PermissionSet};
pub use session::{AuthenticationInfo, DeviceContext, SessionContext};
pub use types::{ApiOperation, OperationType};

