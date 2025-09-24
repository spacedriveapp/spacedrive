//! CQRS (Command Query Responsibility Segregation) for Spacedrive Core
//!
//! This module provides a simplified CQRS implementation that leverages existing
//! infrastructure while providing modular outputs (copying the job system pattern).
//! TODO: Remove Query in favor of CoreQuery and LibraryQuery

use crate::context::CoreContext;
use anyhow::Result;
use std::sync::Arc;

/// A query that retrieves data without mutating state.
///
/// This trait provides the foundation for read-only operations with
/// consistent infrastructure (validation, permissions, logging).
pub trait Query: Send + 'static {
	/// The data structure returned by the query (owned by the operation module).
	type Output: Send + Sync + 'static;
	type Input: Send + Sync + 'static;

	/// Execute this query with the given context.
	fn execute(
		self,
		context: Arc<CoreContext>,
	) -> impl std::future::Future<Output = Result<Self::Output>> + Send;
}

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

	/// Execute this query with the given context and library
	fn execute(
		self,
		context: Arc<CoreContext>,
		library_id: uuid::Uuid,
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

	/// Execute this query with the given context
	fn execute(
		self,
		context: Arc<CoreContext>,
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

	/// Dispatch a query for execution with full infrastructure support
	pub async fn dispatch<Q: Query>(&self, query: Q) -> Result<Q::Output> {
		query.execute(self.context.clone()).await
	}

	/// Dispatch a core-scoped query for execution with full infrastructure support
	pub async fn dispatch_core<Q: CoreQuery>(&self, query: Q) -> Result<Q::Output> {
		query.execute(self.context.clone()).await
	}

	/// Dispatch a library-scoped query for execution with full infrastructure support
	pub async fn dispatch_library<Q: LibraryQuery>(
		&self,
		query: Q,
		library_id: uuid::Uuid,
	) -> Result<Q::Output> {
		query.execute(self.context.clone(), library_id).await
	}
}
