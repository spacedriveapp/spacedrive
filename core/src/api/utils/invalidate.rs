use std::sync::{Arc, Mutex};

use once_cell::sync::OnceCell;
use rspc::{internal::specta::DataType, Type};
use serde::Serialize;
use serde_json::Value;

use crate::api::Router;

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
pub(crate) struct InvalidationRequest {
	pub key: &'static str,
	pub arg_ty: DataType,
	pub macro_src: &'static str,
}

/// invalidation request for a specific resource
#[derive(Debug, Default)]
pub(crate) struct InvalidRequests {
	pub queries: Vec<InvalidationRequest>,
}

impl InvalidRequests {
	#[allow(unused_variables)]
	pub(crate) fn validate(r: Arc<Router>) {
		#[cfg(debug_assertions)]
		{
			let invalidate_requests = crate::api::utils::INVALIDATION_REQUESTS
				.get_or_init(Default::default)
				.lock()
				.unwrap();

			let queries = r.queries();
			for req in &invalidate_requests.queries {
				if let Some(query_ty) = queries.get(req.key) {
					if query_ty.ty.arg_ty != req.arg_ty {
						panic!(
						    "Error at '{}': Attempted to invalid query '{}' but the argument type does not match the type defined on the router.",
						    req.macro_src, req.key
                        );
					}
				} else {
					panic!(
						"Error at '{}': Attempted to invalid query '{}' which was not found in the router",
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
				crate::api::utils::INVALIDATION_REQUESTS
					.get_or_init(|| Default::default())
					.lock()
					.unwrap()
					.queries
					.push(crate::api::utils::InvalidationRequest {
						key: $key,
						arg_ty: <$arg_ty as rspc::internal::specta::Type>::reference(rspc::internal::specta::DefOpts {
                            parent_inline: false,
                            type_map: &mut rspc::internal::specta::TypeDefs::new(),
                        }, &[]),
                        macro_src: concat!(file!(), ":", line!()),
					})
			}
		}

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		let _ = serde_json::to_value($arg)
			.map(|v|
				ctx.emit(crate::api::CoreEvent::InvalidateOperation(
					crate::api::utils::InvalidateOperationEvent::dangerously_create($key, v),
				))
			)
			.map_err(|_| {
				tracing::warn!("Failed to serialize invalidate query event!");
			});
	}};
}
