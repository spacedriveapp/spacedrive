use crate::{CLIENT_CONTEXT, EVENT_SENDER, NODE, RUNTIME};
use jni::objects::{JClass, JObject, JString};
use jni::JNIEnv;
use rspc::Request;
use sdcore::Node;
use tokio::sync::mpsc::unbounded_channel;

#[no_mangle]
pub extern "system" fn Java_com_spacedrive_app_SDCore_registerCoreEventListener(
	env: JNIEnv,
	class: JClass,
) {
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
				},
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
}

#[no_mangle]
pub extern "system" fn Java_com_spacedrive_app_SDCore_handleCoreMsg(
	env: JNIEnv,
	class: JClass,
	query: JString,
	callback: JObject,
) {
	let jvm = env.get_java_vm().unwrap();
	let query: String = env
		.get_string(query)
		.expect("Couldn't get java string!")
		.into();
	let class = env.new_global_ref(class).unwrap();
	let callback = env.new_global_ref(callback).unwrap();

	RUNTIME.spawn(async move {
		let request: Request = serde_json::from_str(&query).unwrap();

		let node = &mut *NODE.lock().await;
		let (node, router) = match node {
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

				let new_node = Node::new(data_dir).await.expect("Unable to create node");
				node.replace(new_node.clone());
				new_node
			},
		};

		let resp = serde_json::to_string(
			&request
				.handle(
					node.get_request_context(),
					&router,
					&CLIENT_CONTEXT,
					EVENT_SENDER.get(),
				)
				.await,
		)
		.unwrap();

		let env = jvm.attach_current_thread().unwrap();
		env.call_method(
			&callback,
			"resolve",
			"(Ljava/lang/Object;)V",
			&[env
				.new_string(resp)
				.expect("Couldn't create java string!")
				.into()],
		)
		.unwrap();
	});
}
