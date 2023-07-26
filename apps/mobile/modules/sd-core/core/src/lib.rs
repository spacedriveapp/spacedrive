use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use once_cell::sync::Lazy;
use rspc::internal::exec::{
	Executor, OwnedStream, Request, StreamOrFut, SubscriptionManager, SubscriptionSet,
};
use sd_core::{api::Ctx, LoggerGuard, Node};
use serde_json::{from_slice, from_str, from_value, to_string, Value};
use std::{
	collections::HashMap,
	ffi::CString,
	marker::Send,
	ops::{Deref, DerefMut},
	path::{Path, PathBuf},
	sync::{Arc, MutexGuard, OnceLock},
};
use tokio::{runtime::Runtime, task::JoinHandle};
use tracing::{debug, error, info};

// Tokio async runtime
static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

// Spacedrive node
type NodeType = Lazy<tokio::sync::Mutex<Option<(Arc<Node>, Executor<Ctx>, LoggerGuard)>>>;
static NODE: NodeType = NodeType::new(|| tokio::sync::Mutex::new(None));

/// EventEmitter is implemented by a channel that can send events to the frontend.
/// This is a trait so Android and IOS can maintain their own implementations.
pub trait EventEmitter: Send + 'static {
	/// Send an event to the frontend.
	fn emit(&self, data: CString);
}

/// Initialise the Spacedrive core
pub fn init_core(data_dir: PathBuf) -> Result<(), String> {
	RUNTIME.block_on(async move {
		debug!(
			"Rust init Spacedrive core in directory '{}'!",
			data_dir.display()
		);

		let state = &mut *NODE.lock().await;

		let guard = Node::init_logger(&data_dir);
		let (node, router) = Node::new(data_dir).await.map_err(|err| err.to_string())?;
		state.replace((node, Executor::new(router), guard));

		Ok(())
	})
}

// State for a single frontend
// For most cases an application will have a single state but with Android multi-window support for Apple watch or something this design could be useful.
// This design is future proofing for that modifying FFI is gross.
pub struct State {
	// connection: (),
	// subscriptions: (),
	// handle: JoinHandle<()>,
}

impl State {
	pub fn new<E: EventEmitter>(ee: E) -> Self {
		let ee = Arc::new(ee);

		// let (clear_subscriptions_tx, mut clear_subscriptions_rx) = mpsc::unbounded_channel();
		// let (tx, rx) = mpsc::unbounded_channel();

		// ConnectionTask::<R, _, _, _>::new(
		// 	(self.ctx_fn)(window.clone()),
		// 	self.executor.clone(),
		// 	Socket {
		// 		recv: rx,
		// 		window: window.clone(),
		// 	},
		// 	Some(Box::new(move |cx| clear_subscriptions_rx.poll_recv(cx))),
		// )

		// let handle = RUNTIME.spawn(async move {
		// 	loop {
		// 		ee.emit(CString::new("Hello World From Rust!").unwrap());
		// 		tokio::time::sleep(std::time::Duration::from_secs(1)).await;
		// 	}
		// });

		Self {}
	}

	pub fn exec(
		&self,
		query: &[u8],
		callback: impl FnOnce(Result<String, String>) + Send + 'static,
	) {
		let reqs = match from_slice::<Value>(query).and_then(|v| match v.is_array() {
			true => from_value::<Vec<Request>>(v),
			false => from_value::<Request>(v).map(|v| vec![v]),
		}) {
			Ok(v) => v,
			Err(err) => {
				callback(Err(format!("failed to decode JSON-RPC request: {}", err)));
				return;
			}
		};

		RUNTIME.spawn(async move {
			// let fut_responses = FuturesUnordered::new();
			// let mut responses = executor.execute_batch(
			// 	&node,
			// 	reqs,
			// 	&mut Some(RnSubscriptionManager(context_id)),
			// 	|fut| fut_responses.push(fut),
			// );
			// responses.append(&mut fut_responses.collect().await);

			// match to_string(&responses) {
			// 	Ok(s) => callback(Ok(s)),
			// 	Err(err) => {
			// 		let err = format!("failed to encode JSON-RPC response: {}", err);
			// 		error!("{err}");
			// 		callback(Err(err));
			// 	}
			// }

			callback(Ok("todo".into()));
		});
	}

	pub fn reset(&self) {
		debug!("Clearing all subscriptions!");

		// TODO
	}

	pub fn shutdown(&self) {
		// self.handle.abort();
	}
}
