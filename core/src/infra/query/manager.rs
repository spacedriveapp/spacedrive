//! Query manager - central router for all queries

use super::error::{QueryError, QueryResult};
use crate::{
	context::CoreContext,
	infra::query::{CoreQuery, LibraryQuery},
};
use std::sync::Arc;
use uuid::Uuid;

/// Central manager for all query execution
///
/// This mirrors the ActionManager pattern but for queries, providing
/// validation, logging, and error handling for read operations.
pub struct QueryManager {
	context: Arc<CoreContext>,
}

impl QueryManager {
	/// Create a new QueryManager
	pub fn new(context: Arc<CoreContext>) -> Self {
		Self { context }
	}

	/// Dispatch a core-scoped query for execution with full infrastructure support
	///
	/// This method:
	/// 1. Validates the query
	/// 2. Logs the query execution
	/// 3. Executes the query with proper error handling
	pub async fn dispatch_core<Q: CoreQuery>(
		&self,
		query: Q,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Q::Output> {
		let query_type = std::any::type_name::<Q>();

		tracing::info!(
			query_type = query_type,
			device_id = %session.auth.device_id,
			"Executing core query"
		);

		// 1. Validate the query
		query.validate(self.context.clone()).await?;

		// 2. Execute query
		let start = std::time::Instant::now();
		let result = query.execute(self.context.clone(), session).await?;
		let duration = start.elapsed();

		tracing::debug!(
			query_type = query_type,
			duration_ms = duration.as_millis(),
			"Core query completed successfully"
		);

		Ok(result)
	}

	/// Dispatch a library-scoped query for execution with full infrastructure support
	///
	/// This method:
	/// 1. Validates library exists
	/// 2. Validates the query
	/// 3. Logs the query execution
	/// 4. Executes the query with proper error handling
	pub async fn dispatch_library<Q: LibraryQuery>(
		&self,
		query: Q,
		library_id: Uuid,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Q::Output> {
		let query_type = std::any::type_name::<Q>();

		tracing::info!(
			query_type = query_type,
			library_id = %library_id,
			device_id = %session.auth.device_id,
			"Executing library query"
		);

		// 1. Get library (query-specific validation)
		let library = self
			.context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(QueryError::LibraryNotFound(library_id))?;

		// 2. Validate the query
		query
			.validate(library.clone(), self.context.clone())
			.await?;

		// 3. Execute query
		let start = std::time::Instant::now();
		let result = query.execute(self.context.clone(), session).await?;
		let duration = start.elapsed();

		tracing::debug!(
			query_type = query_type,
			library_id = %library_id,
			duration_ms = duration.as_millis(),
			"Library query completed successfully"
		);

		Ok(result)
	}
}
