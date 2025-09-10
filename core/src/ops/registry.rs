//! Core-side dynamic registry for actions and queries using `inventory`.
//!
//! This module provides a compile-time, self-registering system for all operations
//! in the Spacedrive core. Operations automatically register themselves using the
//! `inventory` crate, eliminating the need for manual registration boilerplate.
//!
//! ## Architecture
//!
//! The registry system works in three layers:
//! 1. **Registration**: Operations self-register using macros (`register_query!`, `register_action_input!`)
//! 2. **Storage**: Static HashMaps store method-to-handler mappings
//! 3. **Dispatch**: Core engine looks up handlers by method string and executes them
//!
//! ## Usage
//!
//! ```rust
//! // For queries
//! impl Wire for MyQuery {
//!     const METHOD: &'static str = "query:my.domain.v1";
//! }
//! register_query!(MyQuery);
//!
//! // For library actions
//! impl Wire for MyActionInput {
//!     const METHOD: &'static str = "action:my.domain.input.v1";
//! }
//! impl BuildLibraryActionInput for MyActionInput { /* ... */ }
//! register_action_input!(MyActionInput);
//!
//! // For core actions
//! impl BuildCoreActionInput for MyCoreActionInput { /* ... */ }
//! register_core_action_input!(MyCoreActionInput);
//! ```

use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use futures::future::{FutureExt, LocalBoxFuture};
use once_cell::sync::Lazy;

/// Handler function signature for queries.
///
/// Takes a Core instance and serialized query payload, returns serialized result.
/// Uses `LocalBoxFuture` because handlers don't need to be `Send` (they run in the same thread).
pub type QueryHandlerFn =
	fn(Arc<crate::Core>, Vec<u8>) -> LocalBoxFuture<'static, Result<Vec<u8>, String>>;

/// Handler function signature for actions.
///
/// Takes a Core instance, session state, and serialized action payload, returns serialized result.
/// Session state includes things like current library ID and user context.
pub type ActionHandlerFn = fn(
	Arc<crate::Core>,
	crate::infra::daemon::state::SessionState,
	Vec<u8>,
) -> LocalBoxFuture<'static, Result<Vec<u8>, String>>;

/// Registry entry for a query operation.
///
/// Contains the method string (e.g., "query:core.status.v1") and the handler function
/// that will deserialize and execute the query.
pub struct QueryEntry {
	/// The method string used to identify this query
	pub method: &'static str,
	/// The handler function that executes this query
	pub handler: QueryHandlerFn,
}

/// Registry entry for an action operation.
///
/// Contains the method string (e.g., "action:files.copy.input.v1") and the handler function
/// that will deserialize the input, build the action, and execute it.
pub struct ActionEntry {
	/// The method string used to identify this action
	pub method: &'static str,
	/// The handler function that executes this action
	pub handler: ActionHandlerFn,
}

// Collect all registered query and action entries at compile time
inventory::collect!(QueryEntry);
inventory::collect!(ActionEntry);

/// Static HashMap containing all registered query handlers.
///
/// This is lazily initialized on first access. The `inventory` crate automatically
/// collects all `QueryEntry` instances that were registered using `register_query!`
/// and builds this lookup table.
///
/// Key: Method string (e.g., "query:core.status.v1")
/// Value: Handler function that deserializes and executes the query
pub static QUERIES: Lazy<HashMap<&'static str, QueryHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<QueryEntry>() {
		map.insert(entry.method, entry.handler);
	}
	map
});

/// Static HashMap containing all registered action handlers.
///
/// This is lazily initialized on first access. The `inventory` crate automatically
/// collects all `ActionEntry` instances that were registered using `register_action_input!`
/// or `register_core_action_input!` and builds this lookup table.
///
/// Key: Method string (e.g., "action:files.copy.input.v1")
/// Value: Handler function that deserializes input, builds action, and executes it
pub static ACTIONS: Lazy<HashMap<&'static str, ActionHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<ActionEntry>() {
		map.insert(entry.method, entry.handler);
	}
	map
});

/// Generic handler function for executing queries.
///
/// This function is used by the registry to handle all query operations. It:
/// 1. Deserializes the query from the binary payload
/// 2. Executes the query using the Core engine
/// 3. Serializes the result back to binary
///
/// # Type Parameters
/// - `Q`: The query type that implements `Query` trait
///
/// # Arguments
/// - `core`: The Core engine instance
/// - `payload`: Serialized query data
///
/// # Returns
/// - Serialized query result on success
/// - Error string on failure
pub fn handle_query<Q>(
	core: Arc<crate::Core>,
	payload: Vec<u8>,
) -> LocalBoxFuture<'static, Result<Vec<u8>, String>>
where
	Q: crate::cqrs::Query + Serialize + DeserializeOwned + 'static,
	Q::Output: Serialize + 'static,
{
	use bincode::config::standard;
	use bincode::serde::{decode_from_slice, encode_to_vec};
	(async move {
		// Deserialize the query from binary payload
		let q: Q = decode_from_slice(&payload, standard())
			.map_err(|e| e.to_string())?
			.0;

		// Execute the query using the Core engine
		let out: Q::Output = core.execute_query(q).await.map_err(|e| e.to_string())?;

		// Serialize the result back to binary
		encode_to_vec(&out, standard()).map_err(|e| e.to_string())
	})
	.boxed_local()
}

/// Trait for converting external API input types to library actions.
///
/// This trait is implemented by input types (like `FileCopyInput`) that need to be
/// converted to actual library actions (like `FileCopyAction`) before execution.
///
/// Library actions operate within a specific library context and require a library ID.
/// The session state provides the current library ID if not explicitly set in the input.
///
/// # Type Parameters
/// - `Action`: The concrete action type that will be executed
pub trait BuildLibraryActionInput {
	/// The action type that this input builds
	type Action: crate::infra::action::LibraryAction;

	/// Convert the input to an action using session state.
	///
	/// # Arguments
	/// - `session`: Current session state (includes library ID, user context, etc.)
	///
	/// # Returns
	/// - The built action on success
	/// - Error string on failure
	fn build(
		self,
		session: &crate::infra::daemon::state::SessionState,
	) -> Result<Self::Action, String>;
}

/// Trait for converting external API input types to core actions.
///
/// This trait is implemented by input types (like `LibraryCreateInput`) that need to be
/// converted to actual core actions (like `LibraryCreateAction`) before execution.
///
/// Core actions operate at the system level and don't require a specific library context.
/// They can create/delete libraries, manage devices, etc.
///
/// # Type Parameters
/// - `Action`: The concrete action type that will be executed
pub trait BuildCoreActionInput {
	/// The action type that this input builds
	type Action: crate::infra::action::CoreAction;

	/// Convert the input to an action using session state.
	///
	/// # Arguments
	/// - `session`: Current session state (may be used for validation or context)
	///
	/// # Returns
	/// - The built action on success
	/// - Error string on failure
	fn build(
		self,
		session: &crate::infra::daemon::state::SessionState,
	) -> Result<Self::Action, String>;
}

/// Generic handler function for executing library actions.
///
/// This function is used by the registry to handle all library action operations. It:
/// 1. Deserializes the action input from the binary payload
/// 2. Converts the input to a concrete action using the session state
/// 3. Executes the action through the ActionManager
/// 4. Returns an empty result (actions typically don't return data)
///
/// # Type Parameters
/// - `I`: The input type that implements `BuildLibraryActionInput`
///
/// # Arguments
/// - `core`: The Core engine instance
/// - `session`: Current session state (includes library ID, user context)
/// - `payload`: Serialized action input data
///
/// # Returns
/// - Empty vector on success (actions don't return data)
/// - Error string on failure
pub fn handle_library_action_input<I>(
	core: Arc<crate::Core>,
	session: crate::infra::daemon::state::SessionState,
	payload: Vec<u8>,
) -> LocalBoxFuture<'static, Result<Vec<u8>, String>>
where
	I: BuildLibraryActionInput + DeserializeOwned + 'static,
{
	use bincode::config::standard;
	use bincode::serde::decode_from_slice;
	(async move {
		// Deserialize the action input from binary payload
		let input: I = decode_from_slice(&payload, standard())
			.map_err(|e| e.to_string())?
			.0;

		// Convert input to concrete action using session state
		let action = input.build(&session)?;

		// Execute the action through ActionManager
		let action_manager =
			crate::infra::action::manager::ActionManager::new(core.context.clone());
		action_manager
			.dispatch_library(action)
			.await
			.map_err(|e| e.to_string())?;

		// Actions typically don't return data, so return empty vector
		Ok(Vec::new())
	})
	.boxed_local()
}

/// Generic handler function for executing core actions.
///
/// This function is used by the registry to handle all core action operations. It:
/// 1. Deserializes the action input from the binary payload
/// 2. Converts the input to a concrete action using the session state
/// 3. Executes the action through the ActionManager
/// 4. Returns an empty result (actions typically don't return data)
///
/// Core actions operate at the system level (library management, device management, etc.)
/// and don't require a specific library context.
///
/// # Type Parameters
/// - `I`: The input type that implements `BuildCoreActionInput`
///
/// # Arguments
/// - `core`: The Core engine instance
/// - `session`: Current session state (may be used for validation or context)
/// - `payload`: Serialized action input data
///
/// # Returns
/// - Empty vector on success (actions don't return data)
/// - Error string on failure
pub fn handle_core_action_input<I>(
	core: Arc<crate::Core>,
	session: crate::infra::daemon::state::SessionState,
	payload: Vec<u8>,
) -> LocalBoxFuture<'static, Result<Vec<u8>, String>>
where
	I: BuildCoreActionInput + DeserializeOwned + 'static,
{
	use bincode::config::standard;
	use bincode::serde::decode_from_slice;
	(async move {
		// Deserialize the action input from binary payload
		let input: I = decode_from_slice(&payload, standard())
			.map_err(|e| e.to_string())?
			.0;

		// Convert input to concrete action using session state
		let action = input.build(&session)?;

		// Execute the action through ActionManager
		let action_manager =
			crate::infra::action::manager::ActionManager::new(core.context.clone());
		action_manager
			.dispatch_core(action)
			.await
			.map_err(|e| e.to_string())?;

		// Actions typically don't return data, so return empty vector
		Ok(Vec::new())
	})
	.boxed_local()
}

/// Macro for registering query operations with the inventory system.
///
/// This macro automatically registers a query type with the registry, eliminating
/// the need for manual registration boilerplate. The query type must implement
/// the `Wire` trait to provide its method string.
///
/// # Usage
///
/// ```rust
/// impl Wire for MyQuery {
///     const METHOD: &'static str = "query:my.domain.v1";
/// }
/// register_query!(MyQuery);
/// ```
///
/// # What it does
///
/// 1. Creates a `QueryEntry` with the query's method string and handler function
/// 2. Submits the entry to the `inventory` system for compile-time collection
/// 3. The entry will be automatically included in the `QUERIES` HashMap at runtime
#[macro_export]
macro_rules! register_query {
	($ty:ty) => {
		inventory::submit! { $crate::ops::registry::QueryEntry { method: < $ty as $crate::client::Wire >::METHOD, handler: $crate::ops::registry::handle_query::<$ty> } }
	};
}

/// Macro for registering library action input operations with the inventory system.
///
/// This macro automatically registers an action input type with the registry. The
/// input type must implement both `Wire` and `BuildLibraryActionInput` traits.
///
/// # Usage
///
/// ```rust
/// impl Wire for MyActionInput {
///     const METHOD: &'static str = "action:my.domain.input.v1";
/// }
/// impl BuildLibraryActionInput for MyActionInput {
///     type Action = MyAction;
///     fn build(self, session: &SessionState) -> Result<Self::Action, String> { /* ... */ }
/// }
/// register_action_input!(MyActionInput);
/// ```
///
/// # What it does
///
/// 1. Creates an `ActionEntry` with the input's method string and handler function
/// 2. Submits the entry to the `inventory` system for compile-time collection
/// 3. The entry will be automatically included in the `ACTIONS` HashMap at runtime
#[macro_export]
macro_rules! register_action_input {
	($ty:ty) => {
		inventory::submit! { $crate::ops::registry::ActionEntry { method: < $ty as $crate::client::Wire >::METHOD, handler: $crate::ops::registry::handle_library_action_input::<$ty> } }
	};
}

/// Macro for registering core action input operations with the inventory system.
///
/// This macro automatically registers a core action input type with the registry. The
/// input type must implement both `Wire` and `BuildCoreActionInput` traits.
///
/// Core actions operate at the system level (library management, device management, etc.)
/// and don't require a specific library context.
///
/// # Usage
///
/// ```rust
/// impl Wire for MyCoreActionInput {
///     const METHOD: &'static str = "action:my.domain.input.v1";
/// }
/// impl BuildCoreActionInput for MyCoreActionInput {
///     type Action = MyCoreAction;
///     fn build(self, session: &SessionState) -> Result<Self::Action, String> { /* ... */ }
/// }
/// register_core_action_input!(MyCoreActionInput);
/// ```
///
/// # What it does
///
/// 1. Creates an `ActionEntry` with the input's method string and handler function
/// 2. Submits the entry to the `inventory` system for compile-time collection
/// 3. The entry will be automatically included in the `ACTIONS` HashMap at runtime
#[macro_export]
macro_rules! register_core_action_input {
	($ty:ty) => {
		inventory::submit! { $crate::ops::registry::ActionEntry { method: < $ty as $crate::client::Wire >::METHOD, handler: $crate::ops::registry::handle_core_action_input::<$ty> } }
	};
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Test function that lists all registered queries and actions.
	///
	/// This is useful for debugging and verifying that operations are properly
	/// registered with the inventory system.
	///
	/// # Usage
	///
	/// ```rust
	/// #[test]
	/// fn test_list_registered_operations() {
	///     list_registered_operations();
	/// }
	/// ```
	pub fn list_registered_operations() {
		println!("=== Registered Operations ===");

		// List all registered queries
		println!("\n📋 Queries ({} total):", QUERIES.len());
		for (method, _) in QUERIES.iter() {
			println!("  • {}", method);
		}

		// List all registered actions
		println!("\n⚡ Actions ({} total):", ACTIONS.len());
		for (method, _) in ACTIONS.iter() {
			println!("  • {}", method);
		}

		println!("\n=== End Registered Operations ===");
	}

	/// Test function that verifies all registered operations have valid method strings.
	///
	/// This ensures that all registered operations follow the expected naming convention:
	/// - Queries: `query:{domain}.{operation}.v{version}`
	/// - Actions: `action:{domain}.{operation}.input.v{version}`
	#[test]
	fn test_method_naming_convention() {
		// Check query naming convention
		for method in QUERIES.keys() {
			assert!(
				method.starts_with("query:"),
				"Query method '{}' should start with 'query:'",
				method
			);
			assert!(
				method.ends_with(".v1"),
				"Query method '{}' should end with '.v1'",
				method
			);
		}

		// Check action naming convention
		for method in ACTIONS.keys() {
			assert!(
				method.starts_with("action:"),
				"Action method '{}' should start with 'action:'",
				method
			);
			assert!(
				method.ends_with(".input.v1"),
				"Action method '{}' should end with '.input.v1'",
				method
			);
		}
	}

	/// Test function that verifies we have at least some registered operations.
	///
	/// This is a basic smoke test to ensure the inventory system is working
	/// and we have some operations registered.
	#[test]
	fn test_has_registered_operations() {
		// We should have at least the core status query
		assert!(
			QUERIES.contains_key("query:core.status.v1"),
			"Core status query should be registered"
		);

		// We should have at least the libraries list query
		assert!(
			QUERIES.contains_key("query:libraries.list.v1"),
			"Libraries list query should be registered"
		);

		// We should have at least one action registered
		assert!(
			!ACTIONS.is_empty(),
			"Should have at least one action registered"
		);

		// Print the registered operations for debugging
		list_registered_operations();
	}

	/// Test function that verifies no duplicate method strings are registered.
	///
	/// This ensures that each operation has a unique method string.
	#[test]
	fn test_no_duplicate_methods() {
		let mut seen_methods = std::collections::HashSet::new();

		// Check for duplicates in queries
		for method in QUERIES.keys() {
			assert!(
				seen_methods.insert(method),
				"Duplicate query method found: {}",
				method
			);
		}

		// Check for duplicates in actions
		for method in ACTIONS.keys() {
			assert!(
				seen_methods.insert(method),
				"Duplicate action method found: {}",
				method
			);
		}

		// Check for cross-contamination between queries and actions
		for method in QUERIES.keys() {
			assert!(
				!ACTIONS.contains_key(method),
				"Method '{}' is registered as both query and action",
				method
			);
		}
	}
}
