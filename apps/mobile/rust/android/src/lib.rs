use std::panic;

use jni::{
	objects::{JClass, JObject, JString},
	JNIEnv,
};

use sd_core_mobile::*;

use tracing::error;

#[no_mangle]
pub extern "system" fn Java_com_spacedrive_app_SDCore_registerCoreEventListener(
	env: JNIEnv,
	class: JClass,
) {
	let result = panic::catch_unwind(|| {
		let jvm = env.get_java_vm().unwrap();
		let class = env.new_global_ref(class).unwrap();

		spawn_core_event_listener(move |data| {
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
		})
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

		let data_directory = {
			let env = jvm.attach_current_thread().unwrap();
			let data_dir = env
				.call_method(&class, "getDataDirectory", "()Ljava/lang/String;", &[])
				.unwrap()
				.l()
				.unwrap();

			env.get_string(data_dir.into()).unwrap().into()
		};

		handle_core_msg(query, data_directory, move |result| match result {
			Ok(data) => {
				let env = jvm.attach_current_thread().unwrap();
				env.call_method(
					&callback,
					"resolve",
					"(Ljava/lang/Object;)V",
					&[env
						.new_string(data)
						.expect("Couldn't create java string!")
						.into()],
				)
				.unwrap();
			}
			Err(_) => {
				// TODO: handle error
			}
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
