#![cfg(target_os = "ios")]

use std::{
	ffi::{c_char, c_uint, c_void, CStr, CString, OsStr},
	mem::forget,
	os::unix::ffi::OsStrExt,
	panic::catch_unwind,
	path::PathBuf,
};

use sd_mobile_core::{init_core, EventEmitter, State};

extern "C" {
	// TODO: Using `u8` anywhere here is not safe!!!!!! `i8::MAX` is 127 not 255

	fn SDCoreModule_setCoreStartupError(
		sd_core_module: *const c_void,
		len: *const c_uint,
		buf: *const u8,
	);

	fn SDCoreModule_sdEmitEvent(swift_module: *const c_void, len: *const c_uint, buf: *const u8);

	fn SDCoreModule_sdCoreMsgResult(
		swift_module: *const c_void,
		status: *const c_uint,
		len: *const c_uint,
		buf: *const u8,
	);

	fn SDCoreModule_reretainSdCoreModule(swift_module: *const c_void);
}

// An retained reference to the `SDCoreModule` Swift class.
// Will be unset during shutdown.
//
// SAFETY: All methods should hold the mutex lock for the entire duration of it's usage.
// SAFETY: This will ensure the Swift `deinit` method is blocked from releasing the pointer until we are not using the pointer.
pub struct SwiftModule(*mut c_void);

// It's reference counted in Swift, we should be right.
unsafe impl Send for SwiftModule {}

impl EventEmitter for SwiftModule {
	fn emit(&self, data: CString) {
		// todo!();
		// unsafe { SDCoreModule_sdEmitEvent(self.0, data.as_ptr()) };
	}
}

impl Drop for SwiftModule {
	fn drop(&mut self) {
		// todo!();
		// unsafe { SDCoreModule_reretainSdCoreModule(self.0) };
	}
}

/// Intialise the Spacedrive core. This should only be called once on startup.
///
/// SAFETY: `sd_core_module` is an unretained reference so holding on to it after this function is UB.
/// SAFETY: `data_dir` will be deallocated by Swift after this function returns so holding onto it is UB.
#[no_mangle]
pub unsafe extern "C" fn sd_init_core(
	sd_core_module: *const c_void,
	data_dir: *const c_char,
) -> bool {
	let data_dir_path = PathBuf::from(OsStr::from_bytes(CStr::from_ptr(data_dir).to_bytes()));
	match init_core(data_dir_path) {
		Ok(()) => true,
		Err(err) => {
			let buf = err.as_bytes();
			match TryInto::<u32>::try_into(buf.len()) {
				Ok(len) => SDCoreModule_setCoreStartupError(sd_core_module, &len, buf.as_ptr()),
				Err(_) => {
					let fallback: &[u8] = b"ERR_ERROR_STR_LEN_OVERFLOW";
					SDCoreModule_setCoreStartupError(
						sd_core_module,
						// SAFETY: We cast but this is a static string that is way smaller than `u32::MAX`.
						&(fallback.len() as u32),
						fallback.as_ptr(),
					);
				}
			}

			false
		}
	}
}

/// Intialise a new state object for the current frontend.
///
/// SAFETY: `swift_module` is an retained reference so holding onto it is ok but it must be eventually released to Swift to be deallocated.
#[no_mangle]
pub unsafe extern "C" fn sd_init_state(swift_module: *mut c_void) -> *const c_void {
	let module = SwiftModule(swift_module);
	let s = Box::new(State::new(module));
	Box::into_raw(s) as *const c_void
}

/// Deallocate a state object once Swift is done with it.
///
/// This should return the `swift_module` retained pointer back to Swift so it can cleanup but it **doesn't** have to immediately.
///
/// SAFETY: You can take ownership of `state` as Swift must ensure it is never reused after this call.
#[no_mangle]
pub unsafe extern "C" fn sd_deinit_state(state: *mut State) {
	// Drop will cause `State` to abort it's task which holds the `SwiftModule` causing it to drop and pass the `SDCoreModule`reference back to Swift.
	let state = Box::from_raw(state);
	state.shutdown();
	drop(state); // TODO: This may drop it while it's still held by another Rust call because we can make assumptions about how Swift threading works.
}

#[derive(Copy, Clone)]
pub struct SendPtr(*const c_void);

unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

/// Send a message to the core for a given state.
///
/// SAFETY: Ownership must not be taken for `state` or UB with ensue.
/// SAFETY: `query` will be deallocated by Swift after this function returns so holding onto it is UB.
/// SAFETY: `promise` is a retained reference so it can be held onto but it must be eventually released to Swift to be deallocated.
#[no_mangle]
pub unsafe extern "C" fn sd_core_msg(
	state: *mut State,
	query: *const c_char,
	promise: *const c_void,
) {
	let state = Box::from_raw(state);

	let query = CStr::from_ptr(query).to_bytes();
	if let Err(err) = catch_unwind({
		let promise = SendPtr(promise);
		move || {
			// TODO: `state` is hold for longer than the function execution which means it could be freed by another Swift call

			state.exec(query, |result| {
				let (status, string) = match result {
					Ok(result) => (0, result),
					Err(err) => (1, err),
				};

				match TryInto::<u32>::try_into(string.len()) {
					Ok(len) => {
						let promise = promise; // Move whole thing for `unsafe impl Send`

						let a: *const _ = &len;
						let b: *const _ = &status;
						let c: *const _ = string.as_ptr();

						unsafe {
							// SDCoreModule_sdCoreMsgResult(promise.0, b, a, c);
						}
					}
					Err(_) => {
						let fallback: &[u8] = b"ERR_RESULT_AND_ERROR_STR_LEN_OVERFLOW";
						// SDCoreModule_sdCoreMsgResult(
						// 	promise.0,
						// 	&status,
						// 	// SAFETY: We cast but this is a static string that is way smaller than `u32::MAX`.
						// 	&(fallback.len() as u32),
						// 	fallback.as_ptr(),
						// );
					}
				}
			});

			forget(state); // We don't want it to be deallocated
		}
	}) {
		let fallback = b"ERR_RUST_PANICKED";
		SDCoreModule_sdCoreMsgResult(
			promise,
			&2,
			// SAFETY: We cast but this is a static string that is way smaller than `u32::MAX`.
			&(fallback.len() as u32),
			fallback.as_ptr(),
		);
	}
}

/// Reset subscriptions on a state.
///
/// SAFETY: Ownership must not be taken for `state` or UB with ensue.
#[no_mangle]
pub unsafe extern "C" fn sd_state_reset(state: *mut State) {
	let state = Box::from_raw(state);

	state.reset();

	forget(state); // We don't want it to be deallocated
}
