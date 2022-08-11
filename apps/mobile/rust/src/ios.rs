use crate::{NODE, RUNTIME};
use std::{
	ffi::{CStr, CString},
	os::raw::{c_char, c_void},
};

use objc::{class, msg_send, runtime::Object, sel, sel_impl};
use objc_id::Id;
use sdcore::{
	rspc::{ClientContext, Request},
	Node,
};

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
pub unsafe extern "C" fn register_node(id: *mut Id<Object>) {
	// unimplemented!(); // TODO: Finish this

	// let cls = class!(SDCore);
	// let id = Id::<Object>::from_retained_ptr(id);

	// // Bruh
	// let _: () = msg_send![cls, tellJs];
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
			},
		};

		resolve.resolve(
			CString::new(
				serde_json::to_vec(
					&request
						.handle(
							node.get_request_context(),
							&router,
							&ClientContext {
								subscriptions: Default::default(),
							},
							None,
						)
						.await,
				)
				.unwrap(),
			)
			.unwrap(),
		)
	});
}
