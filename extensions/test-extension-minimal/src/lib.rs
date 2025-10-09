//! Absolutely minimal WASM extension
//! No dependencies, just FFI to prove loading works

/// Import host functions
#[link(wasm_import_module = "spacedrive")]
extern "C" {
	fn spacedrive_log(level: u32, msg_ptr: *const u8, msg_len: usize);
}

/// Plugin initialization
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
	let msg = b"Test extension initialized!";
	unsafe {
		spacedrive_log(1, msg.as_ptr(), msg.len());
	}
	0 // Success
}

/// Plugin cleanup
#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 {
	let msg = b"Test extension cleanup";
	unsafe {
		spacedrive_log(1, msg.as_ptr(), msg.len());
	}
	0 // Success
}
