#![cfg(target_os = "android")]

use std::panic;

use jni::{
	objects::{JClass, JObject, JString},
	JNIEnv,
};

use sd_mobile_core::*;

use tracing::error;

#[no_mangle]
pub extern "system" fn Java_com_spacedrive_core_SDCoreModule_registerCoreEventListener(
	env: JNIEnv,
	class: JClass,
) {
	let result = panic::catch_unwind(|| {
		let jvm = env.get_java_vm().unwrap();
		let class = env.new_global_ref(class).unwrap();

		spawn_core_event_listener(move |data| {
			let mut env = jvm.attach_current_thread().unwrap();

			let s = env.new_string(data).expect("Couldn't create java string!");
			env.call_method(
				&class,
				"sendCoreEvent",
				"(Ljava/lang/String;)V",
				&[(&s).into()],
			)
			.unwrap();
		})
	});

	if let Err(err) = result {
		// TODO: Send rspc error or something here so we can show this in the UI.
		// TODO: Maybe reinitialise the core cause it could be in an invalid state?
		error!("Error in Java_com_spacedrive_core_SDCoreModule_registerCoreEventListener: {err:?}");
	}
}

#[no_mangle]
pub extern "system" fn Java_com_spacedrive_core_SDCoreModule_handleCoreMsg(
	env: JNIEnv,
	class: JClass,
	query: JString,
	callback: JObject,
) {
	let jvm = env.get_java_vm().unwrap();
	let mut env = jvm.attach_current_thread().unwrap();
	let callback = env.new_global_ref(callback).unwrap();

	let query: String = env
		.get_string(&query)
		.expect("Couldn't get java string!")
		.into();

	// env.call_method(
	// 	class,
	// 	"printFromRust",
	// 	"(Ljava/lang/Object;)V",
	// 	&[env
	// 		.new_string("Hello from Rust".to_string())
	// 		.expect("Couldn't create java string!")
	// 		.into()],
	// )
	// .unwrap();

	let result = panic::catch_unwind(|| {
		let data_directory = {
			let mut env = jvm.attach_current_thread().unwrap();
			let data_dir = env
				.call_method(&class, "getDataDirectory", "()Ljava/lang/String;", &[])
				.unwrap()
				.l()
				.unwrap();

			env.get_string((&data_dir).into()).unwrap().into()
		};

		let jvm = env.get_java_vm().unwrap();
		handle_core_msg(query, data_directory, move |result| match result {
			Ok(data) => {
				let mut env = jvm.attach_current_thread().unwrap();
				let s = env.new_string(data).expect("Couldn't create java string!");
				env.call_method(
					&callback,
					"resolve",
					"(Ljava/lang/String;)V",
					&[(&s).into()],
				)
				.unwrap();
			}
			Err(err) => error!(err),
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
