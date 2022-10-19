use std::panic;

use crate::{EVENT_SENDER, NODE, RUNTIME, SUBSCRIPTIONS};
use futures::future::join_all;
use jni::objects::{JClass, JObject, JString};
use jni::JNIEnv;
use rspc::internal::jsonrpc::{handle_json_rpc, Request, Sender, SubscriptionMap};
use sd_core::Node;
use serde_json::Value;
use tokio::sync::mpsc::unbounded_channel;
use tracing::{error, info};

#[no_mangle]
pub extern "system" fn Java_com_spacedrive_app_SDCore_registerCoreEventListener(
	env: JNIEnv,
	class: JClass,
) {
	let result = panic::catch_unwind(|| {
		let jvm = env.get_java_vm().unwrap();
		let class = env.new_global_ref(class).unwrap();
		let (tx, mut rx) = unbounded_channel();
		let _ = EVENT_SENDER.set(tx);

		RUNTIME.spawn(async move {
			while let Some(event) = rx.recv().await {
				let data = match serde_json::to_string(&event) {
					Ok(json) => json,
					Err(err) => {
						println!("Failed to serialize event: {}", err);
						continue;
					}
				};

				let env = jvm.attach_current_thread().unwrap();
				env.call_method(
					&class,
					"sendCoreEvent",
					"(Ljava/lang/String;)V",
					&[env
						.new_string(data)
						.expect("Couldn't create java string!")
						.into()],
				)
				.unwrap();
			}
		});
	});

	if let Err(err) = result {
		// TODO: Send rspc error or something here so we can show this in the UI.
		// TODO: Maybe reinitialise the core cause it could be in an invalid state?
		println!(
			"Error in Java_com_spacedrive_app_SDCore_registerCoreEventListener: {:?}",
			err
		);
	}
}

#[no_mangle]
pub extern "system" fn Java_com_spacedrive_app_SDCore_handleCoreMsg(
	env: JNIEnv,
	class: JClass,
	query: JString,
	callback: JObject,
) {
	let result = panic::catch_unwind(|| {
		let jvm = env.get_java_vm().unwrap();
		let query: String = env
			.get_string(query)
			.expect("Couldn't get java string!")
			.into();
		let class = env.new_global_ref(class).unwrap();
		let callback = env.new_global_ref(callback).unwrap();

		RUNTIME.spawn(async move {
			let (node, router) = {
				let node = &mut *NODE.lock().await;
				match node {
					Some(node) => node.clone(),
					None => {
						let data_dir: String = {
							let env = jvm.attach_current_thread().unwrap();
							let data_dir = env
								.call_method(
									&class,
									"getDataDirectory",
									"()Ljava/lang/String;",
									&[],
								)
								.unwrap()
								.l()
								.unwrap();

							env.get_string(data_dir.into()).unwrap().into()
						};

						let new_node = Node::new(data_dir).await;
						let new_node = match new_node {
							Ok(new_node) => new_node,
							Err(err) => {
								info!("677 {:?}", err);

								// TODO: Android return?
								return;
							}
						};

						node.replace(new_node.clone());
						new_node
					}
				}
			};

			let reqs =
				match serde_json::from_str::<Value>(&query).and_then(|v| match v.is_array() {
					true => serde_json::from_value::<Vec<Request>>(v),
					false => serde_json::from_value::<Request>(v).map(|v| vec![v]),
				}) {
					Ok(v) => v,
					Err(err) => {
						error!("failed to decode JSON-RPC request: {}", err); // Don't use tracing here because it's before the `Node` is initialised which sets that config!
						return;
					}
				};

			let resps = join_all(reqs.into_iter().map(|request| {
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

			let env = jvm.attach_current_thread().unwrap();
			env.call_method(
				&callback,
				"resolve",
				"(Ljava/lang/Object;)V",
				&[env
					.new_string(
						serde_json::to_string(
							&resps.into_iter().filter_map(|v| v).collect::<Vec<_>>(),
						)
						.unwrap(),
					)
					.expect("Couldn't create java string!")
					.into()],
			)
			.unwrap();
		});
	});

	if let Err(err) = result {
		// TODO: Send rspc error or something here so we can show this in the UI.
		// TODO: Maybe reinitialise the core cause it could be in an invalid state?

		// TODO: This log statement doesn't work. I recon the JNI env is being dropped before it's called.
		error!(
			"Error in Java_com_spacedrive_app_SDCore_registerCoreEventListener: {:?}",
			err
		);
	}
}
