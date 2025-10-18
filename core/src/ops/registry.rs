//! Minimal action/query registry (action-centric) using `inventory`.
//!
//! Goals:
//! - Tiny, action-centric API: register Actions, decode their associated Inputs
//! - No conversion traits on inputs; Actions declare `type Input` and `from_input(..)`
//! - Single place that resolves `library_id` and dispatches

use futures::future::{FutureExt, LocalBoxFuture};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

/// Registry handler for library queries - thin wrapper calling business logic
pub fn handle_library_query<Q>(
	context: Arc<crate::context::CoreContext>,
	session: crate::infra::api::SessionContext,
	payload: serde_json::Value,
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>
where
	Q: crate::cqrs::LibraryQuery + 'static,
	Q::Input: serde::de::DeserializeOwned + std::fmt::Debug + 'static,
	Q::Output: serde::Serialize + std::fmt::Debug + 'static,
{
	Box::pin(async move {
		// Create dispatcher
		let dispatcher = crate::infra::api::dispatcher::ApiDispatcher::new(context.clone());

		// Deserialize input
		let input: Q::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;

		// Call business logic method
		let output = dispatcher
			.execute_library_query::<Q>(input, session)
			.await
			.map_err(|e| e.to_string())?;

		// Serialize output
		serde_json::to_value(output).map_err(|e| e.to_string())
	})
}

/// Registry handler for core queries - thin wrapper calling business logic
pub fn handle_core_query<Q>(
	context: Arc<crate::context::CoreContext>,
	session: crate::infra::api::SessionContext,
	payload: serde_json::Value,
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>
where
	Q: crate::cqrs::CoreQuery + 'static,
	Q::Input: serde::de::DeserializeOwned + std::fmt::Debug + 'static,
	Q::Output: serde::Serialize + std::fmt::Debug + 'static,
{
	Box::pin(async move {
		// Create dispatcher
		let dispatcher = crate::infra::api::dispatcher::ApiDispatcher::new(context.clone());

		// Deserialize input
		let input: Q::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;

		// Call business logic method
		let output = dispatcher
			.execute_core_query::<Q>(input, session)
			.await
			.map_err(|e| e.to_string())?;

		// Serialize output
		serde_json::to_value(output).map_err(|e| e.to_string())
	})
}

/// Registry handler for library actions - thin wrapper calling business logic
pub fn handle_library_action<A>(
	context: Arc<crate::context::CoreContext>,
	session: crate::infra::api::SessionContext,
	payload: serde_json::Value,
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>
where
	A: crate::infra::action::LibraryAction + 'static,
	A::Input: serde::de::DeserializeOwned + std::fmt::Debug + 'static,
	A::Output: serde::Serialize + std::fmt::Debug + 'static,
{
	Box::pin(async move {
		// Create dispatcher
		let dispatcher = crate::infra::api::dispatcher::ApiDispatcher::new(context.clone());

		// Deserialize input
		let input: A::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;

		// Call business logic method
		let output = dispatcher
			.execute_library_action::<A>(input, session)
			.await
			.map_err(|e| e.to_string())?;

		// Serialize output
		serde_json::to_value(output).map_err(|e| e.to_string())
	})
}

/// Registry handler for core actions - thin wrapper calling business logic
pub fn handle_core_action<A>(
	context: Arc<crate::context::CoreContext>,
	payload: serde_json::Value,
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>
where
	A: crate::infra::action::CoreAction + 'static,
	A::Input: serde::de::DeserializeOwned + std::fmt::Debug + 'static,
	A::Output: serde::Serialize + std::fmt::Debug + 'static,
{
	Box::pin(async move {
		// Create dispatcher
		let dispatcher = crate::infra::api::dispatcher::ApiDispatcher::new(context.clone());

		// Create base session
		let session = dispatcher
			.create_base_session()
			.map_err(|e| e.to_string())?;

		// Deserialize input
		let input: A::Input = serde_json::from_value(payload).map_err(|e| e.to_string())?;

		// Call business logic method
		let output = dispatcher
			.execute_core_action::<A>(input, session)
			.await
			.map_err(|e| e.to_string())?;

		// Serialize output
		serde_json::to_value(output).map_err(|e| e.to_string())
	})
}

/// Handler function signature for library queries.
pub type LibraryQueryHandlerFn = fn(
	Arc<crate::context::CoreContext>,
	crate::infra::api::SessionContext, // session with library context
	serde_json::Value,                 // payload with Q::Input as JSON
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>;

/// Handler function signature for core queries.
pub type CoreQueryHandlerFn = fn(
	Arc<crate::context::CoreContext>,
	crate::infra::api::SessionContext, // session context
	serde_json::Value,                 // payload with Q::Input as JSON
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>;

/// Handler function signature for library actions.
pub type LibraryActionHandlerFn = fn(
	Arc<crate::context::CoreContext>,
	crate::infra::api::SessionContext, // session with library context
	serde_json::Value,                 // payload with A::Input as JSON
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>;

/// Handler function signature for core actions.
pub type CoreActionHandlerFn = fn(
	Arc<crate::context::CoreContext>,
	serde_json::Value, // payload with A::Input as JSON
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'static>,
>;

/// Registry entry for a library query operation.
pub struct LibraryQueryEntry {
	pub method: &'static str,
	pub handler: LibraryQueryHandlerFn,
}

/// Registry entry for a core query operation.
pub struct CoreQueryEntry {
	pub method: &'static str,
	pub handler: CoreQueryHandlerFn,
}

/// Registry entry for a library action operation.
pub struct LibraryActionEntry {
	pub method: &'static str,
	pub handler: LibraryActionHandlerFn,
}

/// Registry entry for a core action operation.
pub struct CoreActionEntry {
	pub method: &'static str,
	pub handler: CoreActionHandlerFn,
}

inventory::collect!(LibraryQueryEntry);
inventory::collect!(CoreQueryEntry);
inventory::collect!(LibraryActionEntry);
inventory::collect!(CoreActionEntry);

pub static LIBRARY_QUERIES: Lazy<HashMap<&'static str, LibraryQueryHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<LibraryQueryEntry>() {
		map.insert(entry.method, entry.handler);
	}
	map
});

pub static CORE_QUERIES: Lazy<HashMap<&'static str, CoreQueryHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<CoreQueryEntry>() {
		map.insert(entry.method, entry.handler);
	}
	map
});

pub static LIBRARY_ACTIONS: Lazy<HashMap<&'static str, LibraryActionHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<LibraryActionEntry>() {
		map.insert(entry.method, entry.handler);
	}
	map
});

pub static CORE_ACTIONS: Lazy<HashMap<&'static str, CoreActionHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<CoreActionEntry>() {
		map.insert(entry.method, entry.handler);
	}
	map
});

#[cfg(test)]
mod tests {
	#[test]
	fn list_registered_ops() {
		// Collect and display registered actions
		let mut action_methods: Vec<&'static str> =
			crate::ops::registry::CORE_ACTIONS.keys().cloned().collect();
		action_methods.sort();
		println!("Registered actions ({}):", action_methods.len());
		for method in &action_methods {
			println!("  {}", method);
		}

		let mut library_action_methods: Vec<&'static str> = crate::ops::registry::LIBRARY_ACTIONS
			.keys()
			.cloned()
			.collect();
		library_action_methods.sort();
		println!(
			"Registered library actions ({}):",
			library_action_methods.len()
		);
		for method in &library_action_methods {
			println!("  {}", method);
		}

		// Collect and display registered queries
		let mut query_methods: Vec<&'static str> =
			crate::ops::registry::CORE_QUERIES.keys().cloned().collect();
		query_methods.sort();
		println!("Registered queries ({}):", query_methods.len());
		for method in &query_methods {
			println!("  {}", method);
		}

		let mut library_query_methods: Vec<&'static str> = crate::ops::registry::LIBRARY_QUERIES
			.keys()
			.cloned()
			.collect();
		library_query_methods.sort();
		println!(
			"Registered library queries ({}):",
			library_query_methods.len()
		);
		for method in &library_query_methods {
			println!("  {}", method);
		}

		// Ensure we have at least one action or query registered
		assert!(
			!action_methods.is_empty()
				|| !query_methods.is_empty()
				|| !library_action_methods.is_empty()
				|| !library_query_methods.is_empty(),
			"No actions or queries registered"
		);
	}
}

/// Helper: construct action method string from a short name like "files.copy"
#[macro_export]
macro_rules! action_method {
	($name:literal) => {
		concat!("action:", $name, ".input.v1")
	};
}

/// Helper: construct query method string from a short name like "core.status"
#[macro_export]
macro_rules! query_method {
	($name:literal) => {
		concat!("query:", $name, ".v1")
	};
}

/// Register a library query `Q` by short name; binds method to `Q::Input` and handler to `handle_library_query::<Q>`.
/// Implements QueryTypeInfo trait for automatic type extraction
#[macro_export]
macro_rules! register_library_query {
	($query:ty, $name:literal) => {
		impl $crate::client::Wire for <$query as $crate::cqrs::LibraryQuery>::Input {
			const METHOD: &'static str = $crate::query_method!($name);
		}
		inventory::submit! {
			$crate::ops::registry::LibraryQueryEntry {
				method: <<$query as $crate::cqrs::LibraryQuery>::Input as $crate::client::Wire>::METHOD,
				handler: $crate::ops::registry::handle_library_query::<$query>,
			}
		}

		// Automatic QueryTypeInfo implementation for type extraction
		impl $crate::ops::type_extraction::QueryTypeInfo for $query {
			type Input = <$query as $crate::cqrs::LibraryQuery>::Input;
			type Output = <$query as $crate::cqrs::LibraryQuery>::Output;

			fn identifier() -> &'static str {
				$name
			}

			fn scope() -> $crate::ops::type_extraction::QueryScope {
				$crate::ops::type_extraction::QueryScope::Library
			}

			fn wire_method() -> String {
				$crate::query_method!($name).to_string()
			}
		}

		// Submit query type extractor to inventory
		inventory::submit! {
			$crate::ops::type_extraction::QueryExtractorEntry {
				extractor: <$query as $crate::ops::type_extraction::QueryTypeInfo>::extract_types,
				identifier: $name,
			}
		}
	};
}

/// Register a core query `Q` by short name; binds method to `Q::Input` and handler to `handle_core_query::<Q>`.
/// Implements QueryTypeInfo trait for automatic type extraction
#[macro_export]
macro_rules! register_core_query {
	($query:ty, $name:literal) => {
		impl $crate::client::Wire for <$query as $crate::cqrs::CoreQuery>::Input {
			const METHOD: &'static str = $crate::query_method!($name);
		}
		inventory::submit! {
			$crate::ops::registry::CoreQueryEntry {
				method: <<$query as $crate::cqrs::CoreQuery>::Input as $crate::client::Wire>::METHOD,
				handler: $crate::ops::registry::handle_core_query::<$query>,
			}
		}

		// Automatic QueryTypeInfo implementation for type extraction
		impl $crate::ops::type_extraction::QueryTypeInfo for $query {
			type Input = <$query as $crate::cqrs::CoreQuery>::Input;
			type Output = <$query as $crate::cqrs::CoreQuery>::Output;

			fn identifier() -> &'static str {
				$name
			}

			fn scope() -> $crate::ops::type_extraction::QueryScope {
				$crate::ops::type_extraction::QueryScope::Core
			}

			fn wire_method() -> String {
				$crate::query_method!($name).to_string()
			}
		}

		// Submit query type extractor to inventory
		inventory::submit! {
			$crate::ops::type_extraction::QueryExtractorEntry {
				extractor: <$query as $crate::ops::type_extraction::QueryTypeInfo>::extract_types,
				identifier: $name,
			}
		}
	};
}

/// Register a library action `A` by short name; binds method to `A::Input` and handler to `handle_library_action::<A>`.
/// Implements OperationTypeInfo trait for automatic type extraction
#[macro_export]
macro_rules! register_library_action {
	($action:ty, $name:literal) => {
		impl $crate::client::Wire for <$action as $crate::infra::action::LibraryAction>::Input {
			const METHOD: &'static str = $crate::action_method!($name);
		}
		inventory::submit! {
			$crate::ops::registry::LibraryActionEntry {
				method: <<$action as $crate::infra::action::LibraryAction>::Input as $crate::client::Wire>::METHOD,
				handler: $crate::ops::registry::handle_library_action::<$action>,
			}
		}

		// Automatic OperationTypeInfo implementation for type extraction
		impl $crate::ops::type_extraction::OperationTypeInfo for $action {
			type Input = <$action as $crate::infra::action::LibraryAction>::Input;
			type Output = <$action as $crate::infra::action::LibraryAction>::Output;

			fn identifier() -> &'static str {
				$name
			}

			fn scope() -> $crate::ops::type_extraction::OperationScope {
				$crate::ops::type_extraction::OperationScope::Library
			}

			fn wire_method() -> String {
				$crate::action_method!($name).to_string()
			}
		}

		// Submit type extractor to inventory for compile-time collection
		inventory::submit! {
			$crate::ops::type_extraction::TypeExtractorEntry {
				extractor: <$action as $crate::ops::type_extraction::OperationTypeInfo>::extract_types,
				identifier: $name,
			}
		}
	};
}

/// Register a core action `A` similarly.
/// Implements OperationTypeInfo trait for automatic type extraction
#[macro_export]
macro_rules! register_core_action {
	($action:ty, $name:literal) => {
		impl $crate::client::Wire for <$action as $crate::infra::action::CoreAction>::Input {
			const METHOD: &'static str = $crate::action_method!($name);
		}
		inventory::submit! {
			$crate::ops::registry::CoreActionEntry {
				method: <<$action as $crate::infra::action::CoreAction>::Input as $crate::client::Wire>::METHOD,
				handler: $crate::ops::registry::handle_core_action::<$action>,
			}
		}

		// Automatic OperationTypeInfo implementation for core actions
		impl $crate::ops::type_extraction::OperationTypeInfo for $action {
			type Input = <$action as $crate::infra::action::CoreAction>::Input;
			type Output = <$action as $crate::infra::action::CoreAction>::Output;

			fn identifier() -> &'static str {
				$name
			}

			fn scope() -> $crate::ops::type_extraction::OperationScope {
				$crate::ops::type_extraction::OperationScope::Core
			}

			fn wire_method() -> String {
				$crate::action_method!($name).to_string()
			}
		}

		// Submit type extractor to inventory for compile-time collection
		inventory::submit! {
			$crate::ops::type_extraction::TypeExtractorEntry {
				extractor: <$action as $crate::ops::type_extraction::OperationTypeInfo>::extract_types,
				identifier: $name,
			}
		}
	};
}
