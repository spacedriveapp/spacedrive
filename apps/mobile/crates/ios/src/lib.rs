#![cfg(target_os = "ios")]

use std::{
	ffi::{CStr, CString},
	os::raw::{c_char, c_void},
	panic,
};

use objc::{msg_send, runtime::Object, sel, sel_impl};
use objc_foundation::{INSString, NSString};
use objc_id::Id;

use sd_mobile_core::*;

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
	let result = panic::catch_unwind(|| {
		let id = Id::<Object>::from_ptr(id);

		spawn_core_event_listener(move |data| {
			let data = NSString::from_str(&data);
			let _: () = msg_send![id, sendCoreEvent: data];
		});
	});

	if let Err(err) = result {
		// TODO: Send rspc error or something here so we can show this in the UI.
		// TODO: Maybe reinitialise the core cause it could be in an invalid state?
		println!("Error in register_core_event_listener: {:?}", err);
	}
}

#[no_mangle]
pub unsafe extern "C" fn sd_core_msg(query: *const c_char, resolve: *const c_void) {
	let result = panic::catch_unwind(|| {
		// This string is cloned to the Rust heap. This is important as Objective-C may remove the query once this function completions but prior to the async block finishing.
		let query = CStr::from_ptr(query).to_str().unwrap().to_string();

		let resolve = RNPromise(resolve);

		let data_directory = CStr::from_ptr(get_data_directory())
			.to_str()
			.unwrap()
			.to_string();

		handle_core_msg(query, data_directory, |result| {
			match result {
				Ok(data) => resolve.resolve(CString::new(data).unwrap()),
				Err(_) => {
					// TODO: handle error
				}
			}
		});
	});

	if let Err(err) = result {
		// TODO: Send rspc error or something here so we can show this in the UI.
		// TODO: Maybe reinitialise the core cause it could be in an invalid state?
		println!("Error in sd_core_msg: {:?}", err);
	}
}
