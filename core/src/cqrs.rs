//! CQRS (Command Query Responsibility Segregation) for Spacedrive Core
//!
//! This module provides a simplified CQRS implementation that leverages existing
//! infrastructure while providing modular outputs (copying the job system pattern).

use crate::context::CoreContext;
use anyhow::Result;
use std::sync::Arc;

/// A query that retrieves data without mutating state.
///
/// This trait provides the foundation for read-only operations with
/// consistent infrastructure (validation, permissions, logging).
pub trait Query {
	/// The data structure returned by the query (owned by the operation module).
	type Output: Send + Sync + 'static;

	/// Execute this query with the given context.
	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output>;
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
		// TODO: Add validation, permissions, audit logging
		// For now, execute directly
		query.execute(self.context.clone()).await
	}
}
