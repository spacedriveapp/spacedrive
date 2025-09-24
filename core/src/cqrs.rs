//! CQRS (Command Query Responsibility Segregation) for Spacedrive Core
//!
//! This module provides a simplified CQRS implementation that leverages existing
//! infrastructure while providing modular outputs (copying the job system pattern).
//! TODO: Remove Query in favor of CoreQuery and LibraryQuery

use crate::context::CoreContext;
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

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

	/// Dispatch a query for execution with full infrastructure support
	pub async fn dispatch<Q: Query>(&self, query: Q) -> Result<Q::Output> {
		query.execute(self.context.clone()).await
	}

	/// Dispatch a core-scoped query for execution with full infrastructure support
	pub async fn dispatch_core<Q: CoreQuery>(&self, query: Q) -> Result<Q::Output> {
		// Create session context for core queries
		let device_id = self.context.device_manager.device_id()?;
		let session = crate::infra::api::SessionContext::device_session(device_id, "Core Device".to_string());
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
		let mut session = crate::infra::api::SessionContext::device_session(device_id, "Core Device".to_string());
		session = session.with_library(library_id);
		query.execute(self.context.clone(), session).await
	}
}
