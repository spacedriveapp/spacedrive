//! Query infrastructure for read-only operations
//!
//! This module provides the query side of our CQRS-inspired architecture:
//! - Query traits (`CoreQuery`, `LibraryQuery`) that operations implement
//! - `QueryManager` for consistent infrastructure (validation, logging)
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
//! use crate::infra::query::LibraryQuery;
//!
//! pub struct DirectoryListingQuery;
//!
//! impl LibraryQuery for DirectoryListingQuery {
//!     type Input = DirectoryListingInput;
//!     type Output = DirectoryListingOutput;
//!
//!     fn from_input(input: Self::Input) -> Result<Self> {
//!         Ok(Self)
//!     }
//!
//!     async fn execute(
//!         self,
//!         context: Arc<CoreContext>,
//!         session: SessionContext,
//!     ) -> Result<Self::Output> {
//!         // Query implementation
//!     }
//! }
//!
//! // Register with wire protocol
//! crate::register_library_query!(DirectoryListingQuery, "files.directory_listing");
//! ```

use crate::context::CoreContext;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

/// Library-scoped query that operates within a specific library context
pub trait LibraryQuery: Send + 'static {
	/// The input data structure for this query
	type Input: Send + Sync + 'static;
	/// The data structure returned by the query
	type Output: Send + Sync + 'static;

	/// Create query from input
	fn from_input(input: Self::Input) -> Result<Self>
	where
		Self: Sized;

	/// Execute this query with rich session context
	///
	/// The session provides authentication, permissions, audit context,
	/// and library context when needed
	fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> impl std::future::Future<Output = Result<Self::Output>> + Send;
}

/// Core-level query that operates at the daemon level
pub trait CoreQuery: Send + 'static {
	/// The input data structure for this query
	type Input: Send + Sync + 'static;
	/// The data structure returned by the query
	type Output: Send + Sync + 'static;

	/// Create query from input
	fn from_input(input: Self::Input) -> Result<Self>
	where
		Self: Sized;

	/// Execute this query with rich session context
	///
	/// The session provides authentication, permissions, and audit context
	fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> impl std::future::Future<Output = Result<Self::Output>> + Send;
}

/// QueryManager provides infrastructure for read-only operations.
///
/// This mirrors the ActionManager pattern but for queries, providing
/// validation, permissions checking, and audit logging for read operations.
pub struct QueryManager {
	context: Arc<CoreContext>,
}

impl QueryManager {
	/// Create a new QueryManager
	pub fn new(context: Arc<CoreContext>) -> Self {
		Self { context }
	}

	/// Dispatch a core-scoped query for execution with full infrastructure support
	pub async fn dispatch_core<Q: CoreQuery>(&self, query: Q) -> Result<Q::Output> {
		// Create session context for core queries
		let device_id = self.context.device_manager.device_id()?;
		let session =
			crate::infra::api::SessionContext::device_session(device_id, "Core Device".to_string());
		query.execute(self.context.clone(), session).await
	}

	/// Dispatch a library-scoped query for execution with full infrastructure support
	pub async fn dispatch_library<Q: LibraryQuery>(
		&self,
		query: Q,
		library_id: Uuid,
	) -> Result<Q::Output> {
		// Create session context for library queries with library context
		let device_id = self.context.device_manager.device_id()?;
		let mut session =
			crate::infra::api::SessionContext::device_session(device_id, "Core Device".to_string());
		session = session.with_library(library_id);
		query.execute(self.context.clone(), session).await
	}
}
