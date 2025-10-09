//! Low-level FFI bindings to Spacedrive host functions
//!
//! This module is internal - extension developers should use the high-level API.

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

/// Register a job with the extension system
///
/// Called automatically by #[extension] macro during plugin_init()
pub fn register_job_with_host(job_name: &str, export_fn: &str, resumable: bool) -> Result<(), ()> {
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
		Err(())
	}
}
