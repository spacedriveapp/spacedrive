//! Low-level FFI bindings to Spacedrive host functions
//!
//! This module is internal - extension developers should use the high-level API.
//!
//! Every host function that returns data goes through [`op`]: the guest names
//! an operation and hands over a payload, the host writes the result into
//! memory it takes from [`wasm_alloc`] and reports where. One convention for
//! every operation keeps the import table small and the memory handling in
//! one place on each side.

use serde::{de::DeserializeOwned, Serialize};

use crate::types::{Error, Result};

// Import Spacedrive host functions
#[link(wasm_import_module = "spacedrive")]
extern "C" {
	fn spacedrive_log(level: u32, msg_ptr: *const u8, msg_len: usize);
	fn register_job(
		job_name_ptr: *const u8,
		job_name_len: u32,
		export_fn_ptr: *const u8,
		export_fn_len: u32,
		resumable: u32,
	) -> i32;
	fn spacedrive_op(
		op_ptr: *const u8,
		op_len: u32,
		payload_ptr: *const u8,
		payload_len: u32,
		out_ptr: *mut u32,
	) -> i32;
}

/// Log a message (info level)
pub fn log_info(message: &str) {
	unsafe {
		spacedrive_log(1, message.as_ptr(), message.len());
	}
}

/// Log a message (debug level)
pub fn log_debug(message: &str) {
	unsafe {
		spacedrive_log(0, message.as_ptr(), message.len());
	}
}

/// Log a message (warn level)
pub fn log_warn(message: &str) {
	unsafe {
		spacedrive_log(2, message.as_ptr(), message.len());
	}
}

/// Log a message (error level)
pub fn log_error(message: &str) {
	unsafe {
		spacedrive_log(3, message.as_ptr(), message.len());
	}
}

/// Memory allocator for host to write results
/// Extension developers don't call this directly - host uses it
#[no_mangle]
pub extern "C" fn wasm_alloc(size: i32) -> *mut u8 {
	let layout = std::alloc::Layout::from_size_align(size as usize, 1).unwrap();
	unsafe { std::alloc::alloc(layout) }
}

/// Free memory allocated by wasm_alloc
#[no_mangle]
pub unsafe extern "C" fn wasm_free(ptr: *mut u8, size: i32) {
	if !ptr.is_null() {
		let layout = std::alloc::Layout::from_size_align(size as usize, 1).unwrap();
		unsafe { std::alloc::dealloc(ptr, layout) };
	}
}

/// Register a job with the extension system
///
/// Called automatically by #[extension] macro during plugin_init()
pub fn register_job_with_host(job_name: &str, export_fn: &str, resumable: bool) -> Result<()> {
	let result = unsafe {
		register_job(
			job_name.as_ptr(),
			job_name.len() as u32,
			export_fn.as_ptr(),
			export_fn.len() as u32,
			if resumable { 1 } else { 0 },
		)
	};

	if result == 0 {
		Ok(())
	} else {
		Err(Error::HostCall(format!("register_job({job_name})")))
	}
}

/// The shape of an error the host reports from an operation.
#[derive(serde::Deserialize)]
struct HostError {
	code: String,
	message: String,
}

/// Run one host operation and return the bytes it answered with.
///
/// The host allocates the answer through `wasm_alloc` and reports pointer and
/// length through `out`; this copies the bytes into a `Vec` and frees the
/// host's buffer, so callers never touch raw guest memory. A non-zero status
/// means the bytes are a JSON error, mapped onto [`Error`] by its code.
pub fn op(op: &str, payload: &[u8]) -> Result<Vec<u8>> {
	let mut out = [0u32; 2];
	let status = unsafe {
		spacedrive_op(
			op.as_ptr(),
			op.len() as u32,
			payload.as_ptr(),
			payload.len() as u32,
			out.as_mut_ptr(),
		)
	};
	let (ptr, len) = (out[0] as *mut u8, out[1] as usize);
	let bytes = if ptr.is_null() || len == 0 {
		Vec::new()
	} else {
		let bytes = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
		unsafe { wasm_free(ptr, len.max(1) as i32) };
		bytes
	};
	if status == 0 {
		return Ok(bytes);
	}
	let error: HostError = serde_json::from_slice(&bytes)
		.map_err(|_| Error::HostCall(format!("{op}: host error {status} was not readable")))?;
	Err(match error.code.as_str() {
		"not_found" => Error::NotFound,
		"permission_denied" => Error::PermissionDenied(error.message),
		"invalid_input" => Error::InvalidInput(error.message),
		"not_available" => Error::NotAvailable(error.message),
		"timeout" => Error::Timeout(error.message),
		"interrupted" => Error::Interrupted,
		_ => Error::OperationFailed(format!("{op}: {}", error.message)),
	})
}

/// [`op`] with a JSON request and a JSON answer.
pub fn op_json<I: Serialize, O: DeserializeOwned>(op_name: &str, input: &I) -> Result<O> {
	let payload = serde_json::to_vec(input).map_err(|e| Error::Serialization(e.to_string()))?;
	let bytes = op(op_name, &payload)?;
	serde_json::from_slice(&bytes).map_err(|e| Error::Deserialization(format!("{op_name}: {e}")))
}

/// Drive a future to completion on the guest's single thread.
///
/// Every host call returns before the guest continues, so a future built from
/// SDK calls is never pending for long. A future that waits on something no
/// host function supplies would spin here; nothing in the SDK produces one.
pub fn block_on<F: core::future::Future>(future: F) -> F::Output {
	let mut future = core::pin::pin!(future);
	let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
	loop {
		if let core::task::Poll::Ready(output) = future.as_mut().poll(&mut cx) {
			return output;
		}
	}
}
