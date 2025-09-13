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

/// Handler function signature for queries.
pub type QueryHandlerFn = fn(
	Arc<crate::Core>,
	Vec<u8>,
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send + 'static>,
>;

/// Handler function signature for actions.
pub type ActionHandlerFn = fn(
	Arc<crate::Core>,
	crate::service::session::SessionState,
	Vec<u8>,
) -> std::pin::Pin<
	Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send + 'static>,
>;

/// Registry entry for a query operation.
pub struct QueryEntry {
	pub method: &'static str,
	pub handler: QueryHandlerFn,
}

/// Registry entry for an action operation.
pub struct ActionEntry {
	pub method: &'static str,
	pub handler: ActionHandlerFn,
}

inventory::collect!(QueryEntry);
inventory::collect!(ActionEntry);

pub static QUERIES: Lazy<HashMap<&'static str, QueryHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<QueryEntry>() {
		map.insert(entry.method, entry.handler);
	}
	map
});

pub static ACTIONS: Lazy<HashMap<&'static str, ActionHandlerFn>> = Lazy::new(|| {
	let mut map = HashMap::new();
	for entry in inventory::iter::<ActionEntry>() {
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
			crate::ops::registry::ACTIONS.keys().cloned().collect();
		action_methods.sort();
		println!("Registered actions ({}):", action_methods.len());
		for method in &action_methods {
			println!("  {}", method);
		}

		// Collect and display registered queries
		let mut query_methods: Vec<&'static str> =
			crate::ops::registry::QUERIES.keys().cloned().collect();
		query_methods.sort();
		println!("Registered queries ({}):", query_methods.len());
		for method in &query_methods {
			println!("  {}", method);
		}

		// Ensure we have at least one action or query registered
		assert!(
			!action_methods.is_empty() || !query_methods.is_empty(),
			"No actions or queries registered"
		);
	}
}

/// Generic query handler (decode -> execute -> encode)
pub fn handle_query<Q>(
	core: Arc<crate::Core>,
	payload: Vec<u8>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send + 'static>>
where
	Q: crate::cqrs::Query + serde::Serialize + DeserializeOwned + 'static,
	Q::Output: serde::Serialize + 'static,
{
	use bincode::config::standard;
	use bincode::serde::{decode_from_slice, encode_to_vec};
	Box::pin(async move {
		let q: Q = decode_from_slice(&payload, standard())
			.map_err(|e| e.to_string())?
			.0;
		let out: Q::Output = core.execute_query(q).await.map_err(|e| e.to_string())?;
		encode_to_vec(&out, standard()).map_err(|e| e.to_string())
	})
}

/// Generic library action handler (decode A::Input -> A::from_input -> dispatch)
pub fn handle_library_action<A>(
	core: Arc<crate::Core>,
	// this isn't used, but is required by the interface, maybe fix?
	session: crate::service::session::SessionState,
	payload: Vec<u8>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send + 'static>>
where
	A: crate::infra::action::LibraryAction + 'static,
	A::Input: DeserializeOwned + 'static,
	A::Output: serde::Serialize + 'static,
{
	use bincode::config::standard;
	use bincode::serde::{decode_from_slice, encode_to_vec};
	Box::pin(async move {
		let input: A::Input = decode_from_slice(&payload, standard())
			.map_err(|e| e.to_string())?
			.0;
		let action = A::from_input(input)?;
		let manager = crate::infra::action::manager::ActionManager::new(core.context.clone());
		let session = core.context.session_state.get().await;
		let library_id = session.current_library_id.ok_or("No library selected")?;
		let out = manager
			.dispatch_library(Some(library_id), action)
			.await
			.map_err(|e| e.to_string())?;
		encode_to_vec(&out, standard()).map_err(|e| e.to_string())
	})
}

/// Generic core action handler (decode A::Input -> A::from_input -> dispatch)
pub fn handle_core_action<A>(
	core: Arc<crate::Core>,
	session: crate::service::session::SessionState,
	payload: Vec<u8>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, String>> + Send + 'static>>
where
	A: crate::infra::action::CoreAction + 'static,
	A::Input: DeserializeOwned + 'static,
	A::Output: serde::Serialize + 'static,
{
	use bincode::config::standard;
	use bincode::serde::{decode_from_slice, encode_to_vec};
	Box::pin(async move {
		let input: A::Input = decode_from_slice(&payload, standard())
			.map_err(|e| e.to_string())?
			.0;
		let action = A::from_input(input)?;
		let manager = crate::infra::action::manager::ActionManager::new(core.context.clone());
		let out = manager
			.dispatch_core(action)
			.await
			.map_err(|e| e.to_string())?;
		encode_to_vec(&out, standard()).map_err(|e| e.to_string())
	})
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

/// Register a query type by action-style name, binding its Wire method automatically.
#[macro_export]
macro_rules! register_query {
	($query:ty, $name:literal) => {
		impl $crate::client::Wire for $query {
			const METHOD: &'static str = $crate::query_method!($name);
		}
		inventory::submit! {
			$crate::ops::registry::QueryEntry {
				method: < $query as $crate::client::Wire >::METHOD,
				handler: $crate::ops::registry::handle_query::<$query>,
			}
		}
	};
}

/// Register a library action `A` by short name; binds method to `A::Input` and handler to `handle_library_action::<A>`.
#[macro_export]
macro_rules! register_library_action {
	($action:ty, $name:literal) => {
		impl $crate::client::Wire for < $action as $crate::infra::action::LibraryAction >::Input {
			const METHOD: &'static str = $crate::action_method!($name);
		}
		inventory::submit! {
			$crate::ops::registry::ActionEntry {
				method: << $action as $crate::infra::action::LibraryAction >::Input as $crate::client::Wire >::METHOD,
				handler: $crate::ops::registry::handle_library_action::<$action>,
			}
		}
	};
}

/// Register a core action `A` similarly.
#[macro_export]
macro_rules! register_core_action {
	($action:ty, $name:literal) => {
		impl $crate::client::Wire for < $action as $crate::infra::action::CoreAction >::Input {
			const METHOD: &'static str = $crate::action_method!($name);
		}
		inventory::submit! {
			$crate::ops::registry::ActionEntry {
				method: << $action as $crate::infra::action::CoreAction >::Input as $crate::client::Wire >::METHOD,
				handler: $crate::ops::registry::handle_core_action::<$action>,
			}
		}
	};
}
