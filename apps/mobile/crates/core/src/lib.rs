use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use once_cell::sync::Lazy;
use rspc::internal::exec::{
	Executor, OwnedStream, Request, StreamOrFut, SubscriptionManager, SubscriptionSet,
};
use sd_core::{api::Ctx, LoggerGuard, Node};
use serde_json::{from_str, from_value, to_string, Value};
use std::{
	collections::HashMap,
	marker::Send,
	ops::{Deref, DerefMut},
	sync::{Arc, MutexGuard, OnceLock},
};
use tokio::runtime::Runtime;
use tracing::error;

// WARNING: rspc "complete" events aren't emitted by this abstraction.
// rspc upstream needs to rethink the `Executor`/`ConnectionTask` relationship cause this doesn't fit either.

// Tokio async runtime
static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

type NodeType = Lazy<tokio::sync::Mutex<Option<(Arc<Node>, Executor<Ctx>, Arc<LoggerGuard>)>>>;

// Spacedrive node + rspc router & executor
static STATE: NodeType = NodeType::new(|| tokio::sync::Mutex::new(None));

// Set of active subscriptions and the task they are running on so they can be cancelled
type SubscriptionState = (SubscriptionSet, HashMap<u32, tokio::task::JoinHandle<()>>);
static SUBSCRIPTIONS: Lazy<std::sync::Mutex<SubscriptionState>> = Lazy::new(Default::default);

// Callback to send a message back to the frontend outside the context of a particular request
static MSG_CALLBACK: OnceLock<Box<dyn Fn(String) + Send + Sync + 'static>> = OnceLock::new();

pub fn set_core_event_listener(callback: impl Fn(String) + Send + Sync + 'static) {
	MSG_CALLBACK.set(Box::new(callback)).ok();
}

pub fn handle_core_msg(
	query: String,
	data_dir: String,
	callback: impl FnOnce(Result<String, String>) + Send + 'static,
) {
	RUNTIME.spawn(async move {
		let (node, executor, _) = {
			let state = &mut *STATE.lock().await;
			match state {
				Some(node) => node.clone(),
				None => {
					let guard = Node::init_logger(&data_dir);

					// TODO: probably don't unwrap
					let (node, router) = Node::new(data_dir).await.unwrap();
					let new_state = (node, Executor::new(router), Arc::new(guard));
					state.replace(new_state.clone());
					new_state
				}
			}
		};

		let reqs = match from_str::<Value>(&query).and_then(|v| match v.is_array() {
			true => from_value::<Vec<Request>>(v),
			false => from_value::<Request>(v).map(|v| vec![v]),
		}) {
			Ok(v) => v,
			Err(err) => {
				error!("failed to decode JSON-RPC request: {}", err);
				callback(Err(query));
				return;
			}
		};

		let fut_responses = FuturesUnordered::new();
		let mut responses =
			executor.execute_batch(&node, reqs, &mut Some(RnSubscriptionManager {}), |fut| {
				fut_responses.push(fut)
			});
		responses.append(&mut fut_responses.collect().await);

		match to_string(&responses) {
			Ok(s) => callback(Ok(s)),
			Err(err) => {
				error!("failed to encode JSON-RPC response: {}", err);
				callback(Err(query));
			}
		}
	});
}

pub struct SubscriptionMutexGuard<'a>(MutexGuard<'a, SubscriptionState>);

impl<'a> Deref for SubscriptionMutexGuard<'a> {
	type Target = SubscriptionSet;

	fn deref(&self) -> &Self::Target {
		&self.0 .0
	}
}

impl<'a> DerefMut for SubscriptionMutexGuard<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0 .0
	}
}

struct RnSubscriptionManager;

impl<TCtx: Clone + Send + 'static> SubscriptionManager<TCtx> for RnSubscriptionManager {
	type Set<'m> = SubscriptionMutexGuard<'m> where Self: 'm;

	fn queue(&mut self, stream: OwnedStream<TCtx>) {
		let id = stream.id;
		let handle = RUNTIME.spawn(
			// We wrap the stream so "complete" messages are handled for us.
			// The need for this is an oversight on rspc API design.
			StreamOrFut::OwnedStream { stream }
				.map(|r| {
					let resp = match serde_json::to_string(&r) {
						Ok(s) => s,
						// This error isn't really handled and that is because if `r` which is a `Response` fails serialization, well we are gonna wanna send a `Response` with the error which will also most likely fail serialization.
						// It's important to note the user provided types are converted to `serde_json::Value` prior to being put into this type so this will only ever fail on internal types.
						Err(err) => {
							error!("rspc internal serialization error: {}", err);

							return;
						}
					};

					if let Some(cb) = MSG_CALLBACK.get() {
						cb(resp);
					} else {
						error!("no RN event callback set! Unable to send response...");
					}
				})
				.into_future()
				.map(|_| ()),
		);

		{
			let mut subscriptions = SUBSCRIPTIONS.lock().unwrap();
			subscriptions.0.insert(id);
			subscriptions.1.insert(id, handle);
		}
	}

	fn subscriptions(&mut self) -> Self::Set<'_> {
		SubscriptionMutexGuard(SUBSCRIPTIONS.lock().unwrap())
	}

	fn abort_subscription(&mut self, id: u32) {
		{
			let mut subscriptions = SUBSCRIPTIONS.lock().unwrap();
			subscriptions.0.remove(&id);
			if let Some(h) = subscriptions.1.remove(&id) {
				h.abort()
			}
		}
	}
}
