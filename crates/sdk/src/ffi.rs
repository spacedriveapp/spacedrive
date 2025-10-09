//! Low-level FFI bindings to Spacedrive host functions
//!
//! This module is internal - extension developers should use the high-level API.

// Import Spacedrive host functions
#[link(wasm_import_module = "spacedrive")]
extern "C" {
	fn spacedrive_log(level: u32, msg_ptr: *const u8, msg_len: usize);
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
