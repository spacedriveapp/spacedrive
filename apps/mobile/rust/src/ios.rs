use crate::{CLIENT_CONTEXT, EVENT_SENDER, NODE, RUNTIME};
use std::{
	ffi::{CStr, CString},
	os::raw::{c_char, c_void},
};
use tokio::sync::mpsc::unbounded_channel;

use objc::{class, msg_send, runtime::Object, sel, sel_impl};
use objc_foundation::{INSString, NSString};
use objc_id::Id;
use rspc::Request;
use sd_core::Node;

extern "C" {
	fn get_data_directory() -> *const c_char;
	fn call_resolve(resolve: *const c_void, result: *const c_char);
}

// This struct wraps the function pointer which represent a Javascript Promise. We wrap the
// function pointers in a struct so we can unsafely assert to Rust that they are `Send`.
// We know they are send as we have ensured Objective-C won't deallocate the function pointer
// until `call_resolve` is called.
struct RNPromise(*const c_void);

unsafe impl Send for RNPromise {}

impl RNPromise {
	// resolve the promise
	unsafe fn resolve(self, result: CString) {
		call_resolve(self.0, result.as_ptr());
	}
}

#[no_mangle]
pub unsafe extern "C" fn register_core_event_listener(id: *mut Object) {
	let id = Id::<Object>::from_ptr(id);

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
			let data = NSString::from_str(&data);
			let _: () = msg_send![id, sendCoreEvent: data];
		}
	});
}

#[no_mangle]
pub unsafe extern "C" fn sd_core_msg(query: *const c_char, resolve: *const c_void) {
	// This string is cloned to the Rust heap. This is important as Objective-C may remove the query once this function completions but prior to the async block finishing.
	let query = CStr::from_ptr(query).to_str().unwrap().to_string();

	let resolve = RNPromise(resolve);
	RUNTIME.spawn(async move {
		let request: Request = serde_json::from_str(&query).unwrap();

		let node = &mut *NODE.lock().await;
		let (node, router) = match node {
			Some(node) => node.clone(),
			None => {
				let doc_dir = CStr::from_ptr(get_data_directory())
					.to_str()
					.unwrap()
					.to_string();
				let new_node = Node::new(doc_dir).await;
				node.replace(new_node.clone());
				new_node
			}
		};

		resolve.resolve(
			CString::new(
				serde_json::to_vec(
					&request
						.handle(
							node.get_request_context(),
							&router,
							&CLIENT_CONTEXT,
							EVENT_SENDER.get(),
						)
						.await,
				)
				.unwrap(),
			)
			.unwrap(),
		)
	});
}
