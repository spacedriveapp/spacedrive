//! Low-level FFI bindings to Spacedrive host functions
//!
//! This module is internal - extension developers should use the high-level API.

use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use crate::types::{Error, Result};

/// Import Spacedrive host functions
#[link(wasm_import_module = "spacedrive")]
extern "C" {
	fn spacedrive_call(
		method_ptr: *const u8,
		method_len: usize,
		library_id_ptr: u32,
		payload_ptr: *const u8,
		payload_len: usize,
	) -> u32;

	fn spacedrive_log(level: u32, msg_ptr: *const u8, msg_len: usize);
}

/// Low-level Wire client (internal use only)
pub struct WireClient {
	library_id: Uuid,
}

impl WireClient {
	pub fn new(library_id: Uuid) -> Self {
		Self { library_id }
	}

	/// Call a Wire operation (generic)
	pub fn call<I, O>(&self, method: &str, input: &I) -> Result<O>
	where
		I: Serialize,
		O: DeserializeOwned,
	{
		// Serialize input to JSON
		let payload =
			serde_json::to_value(input).map_err(|e| Error::Serialization(e.to_string()))?;

		// Call host function
		let result_json = self.call_json(method, Some(self.library_id), payload)?;

		// Deserialize output
		serde_json::from_value(result_json).map_err(|e| Error::Deserialization(e.to_string()))
	}

	/// Call with explicit library ID override
	pub fn call_with_library<I, O>(
		&self,
		method: &str,
		library_id: Option<Uuid>,
		input: &I,
	) -> Result<O>
	where
		I: Serialize,
		O: DeserializeOwned,
	{
		let payload =
			serde_json::to_value(input).map_err(|e| Error::Serialization(e.to_string()))?;
		let result_json = self.call_json(method, library_id, payload)?;
		serde_json::from_value(result_json).map_err(|e| Error::Deserialization(e.to_string()))
	}

	/// Low-level JSON call
	fn call_json(
		&self,
		method: &str,
		library_id: Option<Uuid>,
		payload: serde_json::Value,
	) -> Result<serde_json::Value> {
		// Serialize payload to JSON string
		let payload_json =
			serde_json::to_string(&payload).map_err(|e| Error::Serialization(e.to_string()))?;

		// Prepare library_id pointer (0 = None, or pointer to UUID bytes)
		// Prepare library_id bytes (stored on stack for lifetime)
		let uuid_bytes = library_id.map(|id| *id.as_bytes());
		let lib_id_ptr = match &uuid_bytes {
			None => 0,
			Some(bytes) => bytes.as_ptr() as u32,
		};

		// Call host function
		let result_ptr = unsafe {
			spacedrive_call(
				method.as_ptr(),
				method.len(),
				lib_id_ptr,
				payload_json.as_ptr(),
				payload_json.len(),
			)
		};

		// Check for null (error)
		if result_ptr == 0 {
			return Err(Error::HostCall(
				"Host function returned null (operation failed)".into(),
			));
		}

		// Read result from returned pointer
		// TODO: Implement proper memory reading once host function is complete
		// For now, return a placeholder
		Ok(serde_json::json!({ "placeholder": true }))
	}
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
pub extern "C" fn wasm_free(ptr: *mut u8, size: i32) {
	if !ptr.is_null() {
		let layout = std::alloc::Layout::from_size_align(size as usize, 1).unwrap();
		unsafe { std::alloc::dealloc(ptr, layout) };
	}
}
