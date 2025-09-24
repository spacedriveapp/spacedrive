//! Unified API dispatcher - the main entry point for all operations
//!
//! This is the heart of the API layer. All applications (CLI, GraphQL, Swift)
//! go through this dispatcher to execute operations with proper session context,
//! authentication, and authorization.

use super::{
	error::{ApiError, ApiResult},
	permissions::PermissionLayer,
	session::SessionContext,
};
use crate::{
	context::CoreContext,
	cqrs::{CoreQuery, LibraryQuery},
	infra::action::{CoreAction, LibraryAction},
};
use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use serde::de::DeserializeOwned;
use std::{marker::PhantomData, sync::Arc};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// The unified API dispatcher - main entry point for all operations
///
/// This replaces the scattered handler functions with a single, clean
/// entry point that provides session context, permissions, and audit trails.
#[derive(Clone)]
pub struct ApiDispatcher {
	/// Core context for business logic
	core_context: Arc<CoreContext>,

	/// Permission checking and enforcement
	permission_layer: PermissionLayer,
}

impl ApiDispatcher {
	/// Create a new API dispatcher
	pub fn new(core_context: Arc<CoreContext>) -> Self {
		Self {
			core_context: core_context.clone(),
			permission_layer: PermissionLayer::new(),
		}
	}

	/// Create a permissive dispatcher for development
	pub fn permissive(core_context: Arc<CoreContext>) -> Self {
		Self {
			core_context: core_context.clone(),
			permission_layer: PermissionLayer::permissive(),
		}
	}

	/// Execute a library action with full session context
	///
	/// This is the main entry point for library-scoped operations like
	/// file copy, indexing, tag management, etc.
	pub async fn execute_library_action<A>(
		&self,
		action_input: A::Input,
		session: SessionContext,
	) -> ApiResult<A::Output>
	where
		A: LibraryAction + 'static,
		A::Input: std::fmt::Debug,
		A::Output: std::fmt::Debug,
	{
		// Log the operation start
		info!(
			request_id = %session.request_metadata.request_id,
			action_type = std::any::type_name::<A>(),
			library_id = ?session.current_library_id,
			device_id = %session.auth.device_id,
			"Executing library action"
		);

		// 1. Check permissions
		self.permission_layer
			.check_library_action::<A>(&session, PhantomData)
			.await?;

		// 2. Require library context
		let library_id = session
			.current_library_id
			.ok_or(ApiError::NoLibrarySelected)?;

		// 3. Validate library exists
		let _library = self
			.core_context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(ApiError::LibraryNotFound {
				library_id: library_id.to_string(),
			})?;

		// 4. Create action from input
		let action = A::from_input(action_input).map_err(|e| ApiError::invalid_input(e))?;

		// 5. Execute action with library object
		let result = action
			.execute(_library, self.core_context.clone())
			.await
			.map_err(ApiError::from)?;

		debug!(
			request_id = %session.request_metadata.request_id,
			"Library action completed successfully"
		);

		Ok(result)
	}

	/// Execute a core action with session context
	///
	/// This is for daemon-level operations like library management,
	/// network control, device pairing, etc.
	pub async fn execute_core_action<A>(
		&self,
		action_input: A::Input,
		session: SessionContext,
	) -> ApiResult<A::Output>
	where
		A: CoreAction + 'static,
		A::Input: std::fmt::Debug,
		A::Output: std::fmt::Debug,
	{
		info!(
			request_id = %session.request_metadata.request_id,
			action_type = std::any::type_name::<A>(),
			device_id = %session.auth.device_id,
			"Executing core action"
		);

		// 1. Check permissions
		self.permission_layer
			.check_core_action::<A>(&session, PhantomData)
			.await?;

		// 2. Create action from input
		let action = A::from_input(action_input).map_err(|e| ApiError::invalid_input(e))?;

		// 3. Execute action
		let result = action
			.execute(self.core_context.clone())
			.await
			.map_err(ApiError::from)?;

		debug!(
			request_id = %session.request_metadata.request_id,
			"Core action completed successfully"
		);

		Ok(result)
	}

	/// Execute a library query with session context
	///
	/// This is for library-scoped read operations like file search,
	/// job listing, location listing, etc.
	pub async fn execute_library_query<Q>(
		&self,
		query_input: Q::Input,
		session: SessionContext,
	) -> ApiResult<Q::Output>
	where
		Q: LibraryQuery + 'static,
		Q::Input: std::fmt::Debug,
		Q::Output: std::fmt::Debug,
	{
		debug!(
			request_id = %session.request_metadata.request_id,
			query_type = std::any::type_name::<Q>(),
			library_id = ?session.current_library_id,
			device_id = %session.auth.device_id,
			"Executing library query"
		);

		// 1. Check permissions
		self.permission_layer
			.check_library_query::<Q>(&session, PhantomData)
			.await?;

		// 2. Require library context
		let library_id = session
			.current_library_id
			.ok_or(ApiError::NoLibrarySelected)?;

		// 3. Validate library exists
		let _library = self
			.core_context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or(ApiError::LibraryNotFound {
				library_id: library_id.to_string(),
			})?;

		// 4. Create query from input
		let query =
			Q::from_input(query_input).map_err(|e| ApiError::invalid_input(format!("{}", e)))?;

		// 5. Execute with session context
		let result = query
			.execute(self.core_context.clone(), session.clone())
			.await
			.map_err(|e| ApiError::QueryExecutionFailed {
				reason: format!("{}", e),
			})?;

		Ok(result)
	}

	/// Execute a core query with session context
	///
	/// This is for daemon-level read operations like core status,
	/// library listing, network status, etc.
	pub async fn execute_core_query<Q>(
		&self,
		query_input: Q::Input,
		session: SessionContext,
	) -> ApiResult<Q::Output>
	where
		Q: CoreQuery + 'static,
		Q::Input: std::fmt::Debug,
		Q::Output: std::fmt::Debug,
	{
		debug!(
			request_id = %session.request_metadata.request_id,
			query_type = std::any::type_name::<Q>(),
			device_id = %session.auth.device_id,
			"Executing core query"
		);

		// 1. Check permissions
		self.permission_layer
			.check_core_query::<Q>(&session, PhantomData)
			.await?;

		// 2. Create query from input
		let query =
			Q::from_input(query_input).map_err(|e| ApiError::invalid_input(format!("{}", e)))?;

		// 3. Execute with session context
		let result = query
			.execute(self.core_context.clone(), session.clone())
			.await
			.map_err(|e| ApiError::QueryExecutionFailed {
				reason: format!("{}", e),
			})?;

		Ok(result)
	}

	/// Get a reference to the core context (for advanced usage)
	pub fn core_context(&self) -> &Arc<CoreContext> {
		&self.core_context
	}

	/// Get a reference to the permission layer (for configuration)
	pub fn permission_layer(&self) -> &PermissionLayer {
		&self.permission_layer
	}

	/// Create a base session context for the current device
	pub fn create_base_session(&self) -> Result<crate::infra::api::SessionContext, String> {
		let device_id = self
			.core_context
			.device_manager
			.device_id()
			.map_err(|e| e.to_string())?;
		Ok(crate::infra::api::SessionContext::device_session(
			device_id,
			"Core Device".to_string(),
		))
	}

	/// Library query handler for JSON operations
	pub fn handle_library_query_json<Q>(
		context: Arc<crate::context::CoreContext>,
		session: crate::infra::api::SessionContext,
		payload: serde_json::Value,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
	>
	where
		Q: crate::cqrs::LibraryQuery + 'static,
		Q::Input: serde::de::DeserializeOwned + 'static,
		Q::Output: serde::Serialize + 'static,
	{
		Box::pin(async move {
			let input: Q::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;
			let query = Q::from_input(input).map_err(|e| e.to_string())?;

			let out = query
				.execute(context.clone(), session)
				.await
				.map_err(|e| e.to_string())?;
			serde_json::to_value(out).map_err(|e| e.to_string())
		})
	}

	/// Core query handler for JSON operations
	pub fn handle_core_query_json<Q>(
		context: Arc<crate::context::CoreContext>,
		session: crate::infra::api::SessionContext,
		payload: serde_json::Value,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
	>
	where
		Q: crate::cqrs::CoreQuery + 'static,
		Q::Input: serde::de::DeserializeOwned + 'static,
		Q::Output: serde::Serialize + 'static,
	{
		Box::pin(async move {
			let input: Q::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;
			let query = Q::from_input(input).map_err(|e| e.to_string())?;

			let out = query
				.execute(context.clone(), session)
				.await
				.map_err(|e| e.to_string())?;
			serde_json::to_value(out).map_err(|e| e.to_string())
		})
	}

	/// Library action handler for JSON operations
	pub fn handle_library_action_json<A>(
		context: Arc<crate::context::CoreContext>,
		library_id: Uuid,
		payload: serde_json::Value,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
	>
	where
		A: crate::infra::action::LibraryAction + 'static,
		A::Input: serde::de::DeserializeOwned + 'static,
		A::Output: serde::Serialize + 'static,
	{
		Box::pin(async move {
			let input: A::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;
			let action = A::from_input(input).map_err(|e| e.to_string())?;

			// Get the library object
			let library = context
				.libraries()
				.await
				.get_library(library_id)
				.await
				.ok_or_else(|| "Library not found".to_string())?;

			let out = action
				.execute(library, context.clone())
				.await
				.map_err(|e| e.to_string())?;
			serde_json::to_value(out).map_err(|e| e.to_string())
		})
	}

	/// Core action handler for JSON operations
	pub fn handle_core_action_json<A>(
		context: Arc<crate::context::CoreContext>,
		payload: serde_json::Value,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
	>
	where
		A: crate::infra::action::CoreAction + 'static,
		A::Input: serde::de::DeserializeOwned + 'static,
		A::Output: serde::Serialize + 'static,
	{
		Box::pin(async move {
			let input: A::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;
			let action = A::from_input(input).map_err(|e| e.to_string())?;
			let out = action
				.execute(context.clone())
				.await
				.map_err(|e| e.to_string())?;
			serde_json::to_value(out).map_err(|e| e.to_string())
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::infra::api::session::AuthLevel;

	#[test]
	fn test_api_dispatcher_creation() {
		// This would need a mock CoreContext for testing
		// let core_context = Arc::new(MockCoreContext::new());
		// let dispatcher = ApiDispatcher::new(core_context);
		//
		// assert!(dispatcher.core_context().is_some());

		println!("ApiDispatcher test placeholder - needs mock CoreContext");
	}
}
