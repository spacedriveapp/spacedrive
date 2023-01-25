use futures::future::join_all;
use once_cell::sync::{Lazy, OnceCell};
use rspc::internal::jsonrpc::*;
use sd_core::{api::Router, Node};
use serde_json::{from_str, from_value, to_string, Value};
use std::{collections::HashMap, marker::Send, sync::Arc};
use tokio::{
	runtime::Runtime,
	sync::{
		mpsc::{unbounded_channel, UnboundedSender},
		oneshot, Mutex,
	},
};
use tracing::error;

pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().unwrap());

pub type NodeType = Lazy<Mutex<Option<(Arc<Node>, Arc<Router>)>>>;

pub static NODE: NodeType = Lazy::new(|| Mutex::new(None));

pub static SUBSCRIPTIONS: Lazy<Mutex<HashMap<RequestId, oneshot::Sender<()>>>> =
	Lazy::new(Default::default);

pub static EVENT_SENDER: OnceCell<UnboundedSender<Response>> = OnceCell::new();

pub fn handle_core_msg(
	query: String,
	data_dir: String,
	callback: impl FnOnce(Result<String, String>) + Send + 'static,
) {
	RUNTIME.spawn(async move {
		let (node, router) = {
			let node = &mut *NODE.lock().await;
			match node {
				Some(node) => node.clone(),
				None => {
					// TODO: probably don't unwrap
					let new_node = Node::new(data_dir).await.unwrap();
					node.replace(new_node.clone());
					new_node
				}
			}
		};

		let reqs = match from_str::<Value>(&query).and_then(|v| match v.is_array() {
			true => from_value::<Vec<Request>>(v),
			false => from_value::<Request>(v).map(|v| vec![v]),
		}) {
			Ok(v) => v,
			Err(err) => {
				error!("failed to decode JSON-RPC request: {}", err); // Don't use tracing here because it's before the `Node` is initialised which sets that config!
				callback(Err(query));
				return;
			}
		};

		let responses = join_all(reqs.into_iter().map(|request| {
			let node = node.clone();
			let router = router.clone();
			async move {
				let mut channel = EVENT_SENDER.get().unwrap().clone();
				let mut resp = Sender::ResponseAndChannel(None, &mut channel);

				handle_json_rpc(
					node.get_request_context(),
					request,
					&router,
					&mut resp,
					&mut SubscriptionMap::Mutex(&SUBSCRIPTIONS),
				)
				.await;

				match resp {
					Sender::ResponseAndChannel(resp, _) => resp,
					_ => unreachable!(),
				}
			}
		}))
		.await;

		callback(Ok(serde_json::to_string(
			&responses.into_iter().flatten().collect::<Vec<_>>(),
		)
		.unwrap()));
	});
}

pub fn spawn_core_event_listener(callback: impl Fn(String) + Send + 'static) {
	let (tx, mut rx) = unbounded_channel();
	let _ = EVENT_SENDER.set(tx);

	RUNTIME.spawn(async move {
		while let Some(event) = rx.recv().await {
			let data = match to_string(&event) {
				Ok(json) => json,
				Err(err) => {
					println!("Failed to serialize event: {}", err);
					continue;
				}
			};

			callback(data);
		}
	});
}
