use std::{
	any::TypeId,
	sync::{Arc, Mutex},
};

use once_cell::sync::OnceCell;
use rspc::Type;
use serde::Serialize;
use serde_json::Value;

use super::Router;

/// holds information about all invalidation queries done with the [invalidate_query!] macro so we can check they are valid when building the router.
#[cfg(debug_assertions)]
pub(crate) static INVALIDATION_REQUESTS: OnceCell<Mutex<InvalidRequests>> = OnceCell::new();

#[derive(Debug, Clone, Serialize, Type)]
pub struct InvalidateOperationEvent {
	/// This fields are intentionally private.
	key: &'static str,
	arg: Value,
}

impl InvalidateOperationEvent {
	/// If you are using this function, your doing it wrong.
	pub fn dangerously_create(key: &'static str, arg: Value) -> Self {
		Self { key, arg }
	}
}

/// a request to invalidate a specific resource
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct InvalidationRequest {
	pub key: &'static str,
	pub ty_id: TypeId,
	pub ty_name: &'static str,
	pub macro_src: &'static str,
}

/// invalidation request for a specific resource
#[derive(Debug, Default)]
pub(crate) struct InvalidRequests {
	pub queries: Vec<InvalidationRequest>,
	pub mutations: Vec<InvalidationRequest>,
}

impl InvalidRequests {
	#[allow(unused_variables)]
	pub(crate) fn validate(r: Arc<Router>) {
		#[cfg(debug_assertions)]
		{
			let invalidate_requests = crate::api::invalidate::INVALIDATION_REQUESTS
				.get_or_init(Default::default)
				.lock()
				.unwrap();

			let queries = r.queries();
			for req in &invalidate_requests.queries {
				if !queries.contains_key(req.key) {
					panic!(
						"Error at '{}': Attempted to invalid query '{}' which was not found in the router",
						req.macro_src, req.key
					);
				}
			}

			let mutations = r.mutations();
			for req in &invalidate_requests.mutations {
				if !mutations.contains_key(req.key) {
					panic!(
						"Error at '{}': Attempted to invalid mutation '{}' which was not found in the router",
						req.macro_src, req.key
					);
				}
			}
		}
	}
}

/// invalidate_query is a macro which stores a list of all of it's invocations so it can ensure all of the queries match the queries attached to the router.
/// This allows invalidate to the type safe even when the router keys are stringly typed.
/// ```ignore
/// invalidate_query!(
/// library, // crate::library::LibraryContext
/// "version": (), // Name of the query and the type of it
/// () // The arguments
/// );
/// ```
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! invalidate_query {
	($ctx:expr, $key:literal: $arg_ty:ty, $arg:expr) => {{
		let _: $arg_ty = $arg; // Assert the type the user provided is correct
		let ctx: &crate::library::LibraryContext = &$ctx; // Assert the context is the correct type

		#[cfg(debug_assertions)]
		{
			#[ctor::ctor]
			fn invalidate() {
				crate::api::INVALIDATION_REQUESTS
					.get_or_init(|| Default::default())
					.lock()
					.unwrap()
					.queries
					.push(crate::api::InvalidationRequest {
						key: $key,
						ty_id: std::any::TypeId::of::<$arg_ty>(),
						ty_name: std::any::type_name::<$arg_ty>(),
						macro_src: concat!(file!(), ":", line!()),
					})
			}
		}

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		let _ = serde_json::to_value($arg)
			.map(|v|
				ctx.emit(crate::api::CoreEvent::InvalidateOperation(
					crate::api::InvalidateOperationEvent::dangerously_create($key, v),
				))
			)
			.map_err(|_| {
				tracing::warn!("Failed to serialize invalidate query event!");
			});
	}};
}
