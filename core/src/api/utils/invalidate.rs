use crate::api::{CoreEvent, Ctx, Router, R};

use async_stream::stream;
use rspc::alpha::AlphaRouter;
use serde::Serialize;
use serde_hashkey::to_key;
use serde_json::Value;
use specta::{DataType, Type};
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
	time::Duration,
};
use tokio::sync::broadcast;
use tracing::{debug, warn};

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

	#[allow(unused_variables, clippy::panic)]
	pub(crate) fn validate(r: Arc<Router>) {
		#[cfg(debug_assertions)]
		{
			let invalidate_requests = INVALIDATION_REQUESTS
				.lock()
				.expect("Failed to lock the mutex for invalidation requests");

			let queries = r.queries();
			for req in &invalidate_requests.queries {
				if let Some(query_ty) = queries.get(req.key) {
					if let Some(arg) = &req.arg_ty {
						if &query_ty.ty.input != arg {
							panic!(
								"Error at '{}': Attempted to invalid query '{}' but the argument type does not match the type defined on the router.",
								req.macro_src, req.key
                        	);
						}
					}

					if let Some(result) = &req.result_ty {
						if &query_ty.ty.result != result {
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
		let ctx: &crate::library::LoadedLibrary = &$ctx; // Assert the context is the correct type

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

		::tracing::trace!(target: "sd_core::invalidate-query", "invalidate_query!(\"{}\") at {}", $key, concat!(file!(), ":", line!()));

		// The error are ignored here because they aren't mission critical. If they fail the UI might be outdated for a bit.
		ctx.emit(crate::api::CoreEvent::InvalidateOperation(
			crate::api::utils::InvalidateOperationEvent::dangerously_create($key, serde_json::Value::Null, None)
		))
	}};
	($ctx:expr, $key:literal: $arg_ty:ty, $arg:expr $(,)?) => {{
		let _: $arg_ty = $arg; // Assert the type the user provided is correct
		let ctx: &crate::library::LoadedLibrary = &$ctx; // Assert the context is the correct type

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

		::tracing::trace!(target: "sd_core::invalidate-query", "invalidate_query!(\"{}\") at {}", $key, concat!(file!(), ":", line!()));

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
		let ctx: &crate::library::LoadedLibrary = &$ctx; // Assert the context is the correct type

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

		::tracing::trace!(target: "sd_core::invalidate-query", "invalidate_query!(\"{}\") at {}", $key, concat!(file!(), ":", line!()));

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

pub(crate) fn mount_invalidate() -> AlphaRouter<Ctx> {
	let (tx, _) = broadcast::channel(100);
	let manager_thread_active = Arc::new(AtomicBool::new(false));

	// TODO: Scope the invalidate queries to a specific library (filtered server side)
	let r = if cfg!(debug_assertions) {
		let count = Arc::new(std::sync::atomic::AtomicU16::new(0));

		R.router()
			.procedure(
				"test-invalidate",
				R.query(move |_, _: ()| count.fetch_add(1, Ordering::SeqCst)),
			)
			.procedure(
				"test-invalidate-mutation",
				R.with2(super::library()).mutation(|(_, library), _: ()| {
					invalidate_query!(library, "invalidation.test-invalidate");
					Ok(())
				}),
			)
	} else {
		R.router()
	};

	r.procedure("listen", {
		R.subscription(move |ctx, _: ()| {
			// This thread is used to deal with batching and deduplication.
			// Their is only ever one of these management threads per Node but we spawn it like this so we can steal the event bus from the rspc context.
			// Batching is important because when refetching data on the frontend rspc can fetch all invalidated queries in a single round trip.
			if !manager_thread_active.swap(true, Ordering::Relaxed) {
				let mut event_bus_rx = ctx.event_bus.0.subscribe();
				let tx = tx.clone();
				let manager_thread_active = manager_thread_active.clone();
				tokio::spawn(async move {
					let mut buf = HashMap::with_capacity(100);

					loop {
						tokio::select! {
							event = event_bus_rx.recv() => {
								if let Ok(event) = event {
									if let CoreEvent::InvalidateOperation(op) = event {
										// Newer data replaces older data in the buffer
										match to_key(&(op.key, &op.arg)) {
											Ok(key) => {
												buf.insert(key, op);
											},
											Err(err) => {
												warn!("Error deriving key for invalidate operation '{:?}': {:?}", op, err);
											},
										}

									}
								} else {
									warn!("Shutting down invalidation manager thread due to the core event bus being dropped!");
									break;
								}
							},
							// THROTTLE: Given human reaction time of ~250 milli this should be a good ballance.
							_ = tokio::time::sleep(Duration::from_millis(10)) => {
								let events = buf.drain().map(|(_k, v)| v).collect::<Vec<_>>();
								if !events.is_empty() {
									match tx.send(events) {
										Ok(_) => {},
										// All receivers are shutdown means that all clients are disconnected.
										Err(_) => {
											debug!("Shutting down invalidation manager! This is normal if all clients disconnects.");
											manager_thread_active.swap(false, Ordering::Relaxed);
											break;
										}
									}
								}
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
