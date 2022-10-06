use std::panic;

use crate::{EVENT_SENDER, NODE, RUNTIME, SUBSCRIPTIONS};
use jni::objects::{JClass, JObject, JString};
use jni::JNIEnv;
use rspc::internal::jsonrpc::{handle_json_rpc, Request, Sender, SubscriptionMap};
use sd_core::Node;
use tokio::sync::mpsc::unbounded_channel;

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

		RUNTIME.spawn(async move {
			let request: Request = serde_json::from_str(&query).unwrap();

			let node = &mut *NODE.lock().await;
			let (node, router) = match node {
				Some(node) => node.clone(),
				None => {
					let data_dir: String = {
						let env = jvm.attach_current_thread().unwrap();
						let data_dir = env
							.call_method(&class, "getDataDirectory", "()Ljava/lang/String;", &[])
							.unwrap()
							.l()
							.unwrap();

						env.get_string(data_dir.into()).unwrap().into()
					};

					let new_node = Node::new(data_dir).await.unwrap();
					node.replace(new_node.clone());
					new_node
				}
			};

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
				Sender::Response(Some(resp)) => {
					let env = jvm.attach_current_thread().unwrap();
					env.call_method(
						&callback,
						"resolve",
						"(Ljava/lang/Object;)V",
						&[env
							.new_string(serde_json::to_vec(&resp).unwrap())
							.expect("Couldn't create java string!")
							.into()],
					)
					.unwrap();
				}
				_ => unreachable!(),
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
