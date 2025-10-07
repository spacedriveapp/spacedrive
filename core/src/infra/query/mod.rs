//! Query infrastructure for read-only operations
//!
//! This module provides the query side of our CQRS-inspired architecture:
//! - Query traits (`CoreQuery`, `LibraryQuery`) that operations implement
//! - `QueryManager` for consistent infrastructure (validation, logging, error handling)
//!
//! ## Relationship to Actions
//!
//! Queries are the read-only counterpart to actions (see `infra::action`):
//! - **Queries**: Retrieve data without mutating state
//! - **Actions**: Modify state (create, update, delete)
//!
//! Both use the same wire protocol system (see `infra::wire`) for
//! client-daemon communication.
//!
//! ## Query Types
//!
//! - **`LibraryQuery`**: Queries scoped to a specific library (e.g., file listing)
//! - **`CoreQuery`**: Queries at daemon level (e.g., list all libraries)
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::infra::query::{LibraryQuery, QueryError};
//!
//! pub struct DirectoryListingQuery;
//!
//! impl LibraryQuery for DirectoryListingQuery {
//!     type Input = DirectoryListingInput;
//!     type Output = DirectoryListingOutput;
//!
//!     fn from_input(input: Self::Input) -> Result<Self, QueryError> {
//!         Ok(Self)
//!     }
//!
//!     async fn validate(
//!         &self,
//!         library: Arc<Library>,
//!         context: Arc<CoreContext>,
//!     ) -> Result<(), QueryError> {
//!         // Validation logic here
//!         Ok(())
//!     }
//!
//!     async fn execute(
//!         self,
//!         context: Arc<CoreContext>,
//!         session: SessionContext,
//!     ) -> Result<Self::Output, QueryError> {
//!         // Query implementation
//!     }
//! }
//!
//! // Register with wire protocol
//! crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");
//! ```

use crate::context::CoreContext;
use std::sync::Arc;
use uuid::Uuid;

pub mod context;
pub mod error;
pub mod manager;

pub use error::{QueryError, QueryResult};
pub use manager::QueryManager;

/// Library-scoped query that operates within a specific library context
pub trait LibraryQuery: Send + 'static {
	/// The input data structure for this query
	type Input: Send + Sync + 'static;
	/// The data structure returned by the query
	type Output: Send + Sync + 'static;

	/// Create query from input
	fn from_input(input: Self::Input) -> QueryResult<Self>
	where
		Self: Sized;

	/// Validate the query before execution (optional)
	///
	/// This allows queries to check preconditions, validate paths are within
	/// library bounds, ensure resources exist, etc.
	fn validate(
		&self,
		_library: Arc<crate::library::Library>,
		_context: Arc<CoreContext>,
	) -> impl std::future::Future<Output = Result<(), QueryError>> + Send {
		async { Ok(()) }
	}

	/// Execute this query with rich session context
	///
	/// The session provides authentication, permissions, audit context,
	/// and library context when needed
	fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> impl std::future::Future<Output = QueryResult<Self::Output>> + Send;
}

/// Core-level query that operates at the daemon level
pub trait CoreQuery: Send + 'static {
	/// The input data structure for this query
	type Input: Send + Sync + 'static;
	/// The data structure returned by the query
	type Output: Send + Sync + 'static;

	/// Create query from input
	fn from_input(input: Self::Input) -> QueryResult<Self>
	where
		Self: Sized;

	/// Validate the query before execution (optional)
	///
	/// This allows queries to check preconditions, validate inputs,
	/// ensure resources exist, etc.
	fn validate(
		&self,
		_context: Arc<CoreContext>,
	) -> impl std::future::Future<Output = Result<(), QueryError>> + Send {
		async { Ok(()) }
	}

	/// Execute this query with rich session context
	///
	/// The session provides authentication, permissions, and audit context
	fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> impl std::future::Future<Output = QueryResult<Self::Output>> + Send;
}
