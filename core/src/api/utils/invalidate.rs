use crate::api::{CoreEvent, Router, RouterBuilder};

use async_stream::stream;
use rspc::{internal::specta::DataType, Type};
use serde::Serialize;
use serde_hashkey::to_key;
use serde_json::Value;
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};
use tokio::sync::broadcast;
use tracing::warn;

#[cfg(debug_assertions)]
use std::sync::Mutex;

/// holds information about all invalidation queries done with the [`invalidate_query!`] macro so we can check they are valid when building the router.
#[cfg(debug_assertions)]
pub(crate) static INVALIDATION_REQUESTS: Mutex<InvalidRequests> =
	Mutex::new(InvalidRequests::new());

#[derive(Debug, Clone, Serialize, Type)]
pub struct InvalidateOperationEvent {
	/// This fields are intentionally private.
	key: &'static str,
	arg: Value,
	result: Option<Value>,
}

impl InvalidateOperationEvent {
	/// If you are using this function, your doing it wrong.
	pub fn dangerously_create(key: &'static str, arg: Value, result: Option<Value>) -> Self {
		Self { key, arg, result }
	}
}

/// a request to invalidate a specific resource
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct InvalidationRequest {
	pub key: &'static str,
	pub arg_ty: Option<DataType>,
	pub result_ty: Option<DataType>,
	pub macro_src: &'static str,
}

/// invalidation request for a specific resource
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct InvalidRequests {
	pub queries: Vec<InvalidationRequest>,
}

impl InvalidRequests {
	#[allow(unused)]
	const fn new() -> Self {
		Self {
			queries: Vec::new(),
		}
	}

	#[allow(unused_variables)]
	pub(crate) fn validate(r: Arc<Router>) {
		#[cfg(debug_assertions)]
		{
			let invalidate_requests = INVALIDATION_REQUESTS.lock().unwrap();

			let queries = r.queries();
			for req in &invalidate_requests.queries {
				if let Some(query_ty) = queries.get(req.key) {
					if let Some(arg) = &req.arg_ty {
						if &query_ty.ty.arg_ty != arg {
							panic!(
								"Error at '{}': Attempted to invalid query '{}' but the argument type does not match the type defined on the router.",
								req.macro_src, req.key
                        	);
						}
					}

					if let Some(result) = &req.result_ty {
						if &query_ty.ty.result_ty != result {
							panic!(
								"Error at '{}': Attempted to invalid query '{}' but the data type does not match the type defined on the router.",
								req.macro_src, req.key
                        	);
						}
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

/// `invalidate_query` is a macro which stores a list of all of it's invocations so it can ensure all of the queries match the queries attached to the router.
/// This allows invalidate to be type-safe even when the router keys are stringly typed.
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
	($ctx:expr, $key:literal) => {{
		let ctx: &crate::library::LibraryContext = &$ctx; // Assert the context is the correct type

		#[cfg(debug_assertions)]
		{
			#[ctor::ctor]
			fn invalidate() {
				crate::api::utils::INVALIDATION_REQUESTS
					.lock()
					.unwrap()
					.queries
					.push(crate::api::utils::InvalidationRequest {
						key: $key,
						arg_ty: None,
						result_ty: None,
            			macro_src: concat!(file!(), ":", line!()),
					})
			}
		}

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		ctx.emit(crate::api::CoreEvent::InvalidateOperation(
			crate::api::utils::InvalidateOperationEvent::dangerously_create($key, serde_json::Value::Null, None)
		))
	}};
	($ctx:expr, $key:literal: $arg_ty:ty, $arg:expr $(,)?) => {{
		let _: $arg_ty = $arg; // Assert the type the user provided is correct
		let ctx: &crate::library::LibraryContext = &$ctx; // Assert the context is the correct type

		#[cfg(debug_assertions)]
		{
			#[ctor::ctor]
			fn invalidate() {
				crate::api::utils::INVALIDATION_REQUESTS
					.lock()
					.unwrap()
					.queries
					.push(crate::api::utils::InvalidationRequest {
						key: $key,
						arg_ty: Some(<$arg_ty as rspc::internal::specta::Type>::reference(rspc::internal::specta::DefOpts {
                            parent_inline: false,
                            type_map: &mut rspc::internal::specta::TypeDefs::new(),
                        }, &[])),
						result_ty: None,
                        macro_src: concat!(file!(), ":", line!()),
					})
			}
		}

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		let _ = serde_json::to_value($arg)
			.map(|v|
				ctx.emit(crate::api::CoreEvent::InvalidateOperation(
					crate::api::utils::InvalidateOperationEvent::dangerously_create($key, v, None),
				))
			)
			.map_err(|_| {
				tracing::warn!("Failed to serialize invalidate query event!");
			});
	}};
	($ctx:expr, $key:literal: $arg_ty:ty, $arg:expr, $result_ty:ty: $result:expr $(,)?) => {{
		let _: $arg_ty = $arg; // Assert the type the user provided is correct
		let ctx: &crate::library::LibraryContext = &$ctx; // Assert the context is the correct type

		#[cfg(debug_assertions)]
		{
			#[ctor::ctor]
			fn invalidate() {
				crate::api::utils::INVALIDATION_REQUESTS
					.lock()
					.unwrap()
					.queries
					.push(crate::api::utils::InvalidationRequest {
						key: $key,
						arg_ty: Some(<$arg_ty as rspc::internal::specta::Type>::reference(rspc::internal::specta::DefOpts {
                            parent_inline: false,
                            type_map: &mut rspc::internal::specta::TypeDefs::new(),
                        }, &[])),
						result_ty: Some(<$result_ty as rspc::internal::specta::Type>::reference(rspc::internal::specta::DefOpts {
                            parent_inline: false,
                            type_map: &mut rspc::internal::specta::TypeDefs::new(),
                        }, &[])),
                        macro_src: concat!(file!(), ":", line!()),
					})
			}
		}

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		let _ = serde_json::to_value($arg)
			.and_then(|arg|
				serde_json::to_value($result)
				.map(|result|
					ctx.emit(crate::api::CoreEvent::InvalidateOperation(
						crate::api::utils::InvalidateOperationEvent::dangerously_create($key, arg, Some(result)),
					))
				)
			)
			.map_err(|_| {
				tracing::warn!("Failed to serialize invalidate query event!");
			});
	}};
}

pub fn mount_invalidate() -> RouterBuilder {
	let (tx, _) = broadcast::channel(100);
	let manager_thread_active = AtomicBool::new(false);

	// TODO: Scope the invalidate queries to a specific library (filtered server side)
	RouterBuilder::new().subscription("listen", move |t| {
		t(move |ctx, _: ()| {
			// This thread is used to deal with batching and deduplication.
			// Their is only ever one of these management threads per Node but we spawn it like this so we can steal the event bus from the rspc context.
			// Batching is important because when refetching data on the frontend rspc can fetch all invalidated queries in a single round trip.
			if !manager_thread_active.swap(true, Ordering::Relaxed) {
				println!("TODO: STARTING");

				let mut event_bus_rx = ctx.event_bus.subscribe();
				let tx = tx.clone();
				tokio::spawn(async move {
					let mut buf = HashMap::with_capacity(100);

					tokio::select! {
						event = event_bus_rx.recv() => {
							if let Ok(event) = event {
								match event {
									CoreEvent::InvalidateOperation(op) => {
										// Newer data replaces older data in the buffer
										let key = to_key(&(op.key, &op.arg)).unwrap();
										if buf.get(&key).is_some() {
											buf.remove(&key);
										}
										buf.insert(key, op);
									}
									_ => {}
								}
							} else {
								warn!("Shutting down invalidation manager thread due to the core event bus being droppped!");
								return;
							}
						},
						// Given human reaction time of ~250 milli this should be a good ballance.
						_ = tokio::time::sleep(Duration::from_millis(200)) => {
							match tx.send(buf.drain().map(|(_k, v)| v).collect::<Vec<_>>()) {
								Ok(_) => {},
								Err(_) => warn!("Error emitting invalidation manager events!"),
							}
						}
					}
				});
			}

			let mut rx = tx.subscribe();
			stream! {
				while let Ok(msg) = rx.recv().await {
					yield msg;
				}
			}
		})
	})
}
