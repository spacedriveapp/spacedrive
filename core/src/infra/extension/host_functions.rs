//! WASM host functions
//!
//! This module provides the bridge between WASM extensions and Spacedrive's
//! operation registry. The key function is `host_spacedrive_call()` which routes
//! generic Wire method calls to the existing `execute_json_operation()` function
//! used by daemon RPC.

use std::sync::Arc;

use uuid::Uuid;
use wasmer::{FunctionEnvMut, Memory, MemoryView, WasmPtr};

use crate::{infra::daemon::rpc::RpcServer, Core};

use super::permissions::ExtensionPermissions;

/// Environment passed to all host functions
pub struct PluginEnv {
	pub extension_id: String,
	pub core_context: Arc<crate::context::CoreContext>, // Just context, not full Core!
	pub api_dispatcher: Arc<crate::infra::api::ApiDispatcher>, // For creating sessions
	pub permissions: ExtensionPermissions,
	pub memory: Memory,
	pub job_registry: Arc<super::job_registry::ExtensionJobRegistry>,
}

/// THE MAIN HOST FUNCTION - Generic Wire RPC
///
/// This is the ONLY function WASM extensions need to call Spacedrive operations.
/// It routes calls to the existing Wire operation registry.
///
/// # Arguments
/// - `method_ptr`, `method_len`: Wire method string (e.g., "query:ai.ocr.v1")
/// - `library_id_ptr`: 0 for None, or pointer to 16 UUID bytes
/// - `payload_ptr`, `payload_len`: JSON payload string
///
/// # Returns
/// Pointer to result JSON string in WASM memory (or 0 on error)
pub fn host_spacedrive_call(
	mut env: FunctionEnvMut<PluginEnv>,
	method_ptr: WasmPtr<u8>,
	method_len: u32,
	library_id_ptr: u32,
	payload_ptr: WasmPtr<u8>,
	payload_len: u32,
) -> u32 {
	let (plugin_env, mut store) = env.data_and_store_mut();

	// Get memory view from environment
	let memory = &plugin_env.memory;
	let memory_view = memory.view(&store);

	// 1. Read method string from WASM memory
	let method = match read_string_from_wasm(&memory_view, method_ptr, method_len) {
		Ok(m) => m,
		Err(e) => {
			tracing::error!("Failed to read method string: {}", e);
			return 0;
		}
	};

	// 2. Read library_id (0 = None)
	let library_id = if library_id_ptr == 0 {
		None
	} else {
		match read_uuid_from_wasm(&memory_view, WasmPtr::new(library_id_ptr)) {
			Ok(uuid) => Some(uuid),
			Err(e) => {
				tracing::error!("Failed to read library UUID: {}", e);
				return 0;
			}
		}
	};

	// 3. Read payload JSON
	let payload_str = match read_string_from_wasm(&memory_view, payload_ptr, payload_len) {
		Ok(s) => s,
		Err(e) => {
			tracing::error!("Failed to read payload: {}", e);
			return 0;
		}
	};

	let payload_json: serde_json::Value = match serde_json::from_str(&payload_str) {
		Ok(json) => json,
		Err(e) => {
			tracing::error!("Failed to parse payload JSON: {}", e);
			return write_error_to_memory(&memory, &mut store, &format!("Invalid JSON: {}", e));
		}
	};

	// 4. Permission check
	let auth_result = tokio::runtime::Handle::current()
		.block_on(async { plugin_env.permissions.authorize(&method, library_id).await });

	if let Err(e) = auth_result {
		tracing::warn!(
			extension = %plugin_env.extension_id,
			method = %method,
			"Permission denied: {}",
			e
		);
		return write_error_to_memory(&memory, &mut store, &format!("Permission denied: {}", e));
	}

	tracing::debug!(
		extension = %plugin_env.extension_id,
		method = %method,
		library_id = ?library_id,
		"Extension calling operation"
	);

	// 5. Call operation handlers directly (same as execute_json_operation does)
	let result = tokio::runtime::Handle::current().block_on(async {
		// Create base session
		let base_session = match plugin_env.api_dispatcher.create_base_session() {
			Ok(s) => s,
			Err(e) => return Err(e),
		};

		// Try library queries
		if let Some(handler) = crate::infra::wire::registry::LIBRARY_QUERIES.get(method.as_str()) {
			let lib_id = library_id.ok_or_else(|| "Library ID required".to_string())?;
			let session = base_session.with_library(lib_id);
			return handler(plugin_env.core_context.clone(), session, payload_json).await;
		}

		// Try core queries
		if let Some(handler) = crate::infra::wire::registry::CORE_QUERIES.get(method.as_str()) {
			return handler(plugin_env.core_context.clone(), base_session, payload_json).await;
		}

		// Try library actions
		if let Some(handler) = crate::infra::wire::registry::LIBRARY_ACTIONS.get(method.as_str()) {
			let lib_id = library_id.ok_or_else(|| "Library ID required".to_string())?;
			let session = base_session.with_library(lib_id);
			return handler(plugin_env.core_context.clone(), session, payload_json).await;
		}

		// Try core actions
		if let Some(handler) = crate::infra::wire::registry::CORE_ACTIONS.get(method.as_str()) {
			return handler(plugin_env.core_context.clone(), payload_json).await;
		}

		Err(format!("Unknown method: {}", method))
	});

	// 6. Write result to WASM memory
	match result {
		Ok(json) => write_json_to_memory(&memory, &mut store, &json),
		Err(e) => {
			tracing::error!("Operation failed: {}", e);
			write_error_to_memory(&memory, &mut store, &e)
		}
	}
}

/// Optional logging helper for extensions
pub fn host_spacedrive_log(
	mut env: FunctionEnvMut<PluginEnv>,
	level: u32,
	msg_ptr: WasmPtr<u8>,
	msg_len: u32,
) {
	let (plugin_env, mut store) = env.data_and_store_mut();

	// Get memory view from environment
	let memory = &plugin_env.memory;
	let memory_view = memory.view(&store);

	let message = match read_string_from_wasm(&memory_view, msg_ptr, msg_len) {
		Ok(msg) => msg,
		Err(_) => {
			tracing::error!("Failed to read log message from WASM");
			return;
		}
	};

	match level {
		0 => tracing::debug!(extension = %plugin_env.extension_id, "{}", message),
		1 => tracing::info!(extension = %plugin_env.extension_id, "{}", message),
		2 => tracing::warn!(extension = %plugin_env.extension_id, "{}", message),
		3 => tracing::error!(extension = %plugin_env.extension_id, "{}", message),
		_ => tracing::info!(extension = %plugin_env.extension_id, "{}", message),
	}
}

// === Memory Helpers ===

fn read_string_from_wasm(
	memory_view: &MemoryView,
	ptr: WasmPtr<u8>,
	len: u32,
) -> Result<String, Box<dyn std::error::Error>> {
	let bytes = ptr
		.slice(memory_view, len)
		.and_then(|slice| slice.read_to_vec())
		.map_err(|e| format!("Failed to read from WASM memory: {:?}", e))?;

	String::from_utf8(bytes).map_err(|e| e.into())
}

fn read_uuid_from_wasm(
	memory_view: &MemoryView,
	ptr: WasmPtr<u8>,
) -> Result<Uuid, Box<dyn std::error::Error>> {
	let bytes = ptr
		.slice(memory_view, 16)
		.and_then(|slice| slice.read_to_vec())
		.map_err(|e| format!("Failed to read UUID from WASM memory: {:?}", e))?;

	let uuid_bytes: [u8; 16] = bytes
		.try_into()
		.map_err(|_| "Invalid UUID bytes (expected 16 bytes)")?;

	Ok(Uuid::from_bytes(uuid_bytes))
}

fn write_json_to_memory(
	memory: &Memory,
	store: &mut wasmer::StoreMut,
	json: &serde_json::Value,
) -> u32 {
	let json_str = match serde_json::to_string(json) {
		Ok(s) => s,
		Err(e) => {
			tracing::error!("Failed to serialize JSON: {}", e);
			return 0; // NULL indicates error
		}
	};

	let bytes = json_str.as_bytes();

	// Try to call guest's allocator function
	// WASM module must export: fn wasm_alloc(size: i32) -> i32
	let alloc_result = memory
		.view(&store)
		.data_size() // Just check memory exists for now
		.checked_sub(bytes.len() as u64);

	if alloc_result.is_none() {
		tracing::error!("Not enough WASM memory for result");
		return 0;
	}

	// For now, write to a fixed offset (will implement proper allocator later)
	// This is a simplification for testing - production needs guest allocator
	let result_offset = 65536u32; // Start at 64KB

	let memory_view = memory.view(&store);
	let wasm_ptr = WasmPtr::<u8>::new(result_offset);

	if let Ok(slice) = wasm_ptr.slice(&memory_view, bytes.len() as u32) {
		if let Err(e) = slice.write_slice(bytes) {
			tracing::error!("Failed to write to WASM memory: {:?}", e);
			return 0;
		}
	} else {
		tracing::error!("Failed to get WASM memory slice");
		return 0;
	}

	result_offset
}

fn write_error_to_memory(memory: &Memory, store: &mut wasmer::StoreMut, error: &str) -> u32 {
	let error_json = serde_json::json!({ "error": error });
	write_json_to_memory(memory, store, &error_json)
}

// === Job-Specific Host Functions ===

/// Report job progress
pub fn host_job_report_progress(
	mut env: FunctionEnvMut<PluginEnv>,
	job_id_ptr: WasmPtr<u8>,
	progress: f32,
	message_ptr: WasmPtr<u8>,
	message_len: u32,
) {
	let (plugin_env, mut store) = env.data_and_store_mut();
	let memory = &plugin_env.memory;
	let memory_view = memory.view(&store);

	let job_id = match read_uuid_from_wasm(&memory_view, job_id_ptr) {
		Ok(id) => id,
		Err(e) => {
			tracing::error!("Failed to read job ID: {}", e);
			return;
		}
	};

	let message = match read_string_from_wasm(&memory_view, message_ptr, message_len) {
		Ok(msg) => msg,
		Err(e) => {
			tracing::error!("Failed to read message: {}", e);
			return;
		}
	};

	tracing::info!(
		job_id = %job_id,
		progress = %progress,
		extension = %plugin_env.extension_id,
		"{}",
		message
	);

	// TODO: Forward to actual JobContext once registry is implemented
}

/// Save job checkpoint
pub fn host_job_checkpoint(
	mut env: FunctionEnvMut<PluginEnv>,
	job_id_ptr: WasmPtr<u8>,
	_state_ptr: WasmPtr<u8>,
	_state_len: u32,
) -> i32 {
	let (plugin_env, mut store) = env.data_and_store_mut();
	let memory = &plugin_env.memory;
	let memory_view = memory.view(&store);

	let job_id = match read_uuid_from_wasm(&memory_view, job_id_ptr) {
		Ok(id) => id,
		Err(e) => {
			tracing::error!("Failed to read job ID: {}", e);
			return 1; // Error
		}
	};

	tracing::debug!(job_id = %job_id, extension = %plugin_env.extension_id, "Checkpoint saved");

	// TODO: Actually save state to database
	0 // Success
}

/// Check if job should be interrupted
pub fn host_job_check_interrupt(
	mut env: FunctionEnvMut<PluginEnv>,
	job_id_ptr: WasmPtr<u8>,
) -> i32 {
	let (plugin_env, mut store) = env.data_and_store_mut();
	let memory = &plugin_env.memory;
	let memory_view = memory.view(&store);

	let _job_id = match read_uuid_from_wasm(&memory_view, job_id_ptr) {
		Ok(id) => id,
		Err(e) => {
			tracing::error!("Failed to read job ID: {}", e);
			return 0; // Continue
		}
	};

	// TODO: Check actual interrupt status
	0 // Not interrupted
}

/// Add job warning
pub fn host_job_add_warning(
	mut env: FunctionEnvMut<PluginEnv>,
	job_id_ptr: WasmPtr<u8>,
	message_ptr: WasmPtr<u8>,
	message_len: u32,
) {
	let (plugin_env, mut store) = env.data_and_store_mut();
	let memory = &plugin_env.memory;
	let memory_view = memory.view(&store);

	let job_id = match read_uuid_from_wasm(&memory_view, job_id_ptr) {
		Ok(id) => id,
		Err(_) => return,
	};

	let message = match read_string_from_wasm(&memory_view, message_ptr, message_len) {
		Ok(msg) => msg,
		Err(_) => return,
	};

	tracing::warn!(job_id = %job_id, extension = %plugin_env.extension_id, "Job warning: {}", message);
}

/// Increment bytes processed
pub fn host_job_increment_bytes(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
	bytes: u64,
) {
	let (plugin_env, _store) = env.data_and_store_mut();
	tracing::debug!(extension = %plugin_env.extension_id, "Processed {} bytes", bytes);
	// TODO: Update metrics
}

/// Increment items processed
pub fn host_job_increment_items(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
	count: u64,
) {
	let (plugin_env, _store) = env.data_and_store_mut();
	tracing::debug!(extension = %plugin_env.extension_id, "Processed {} items", count);
	// TODO: Update metrics
}

// === Extension Registration Functions ===

/// Register a job type for an extension
///
/// Called from plugin_init() to register custom job types
///
/// # Arguments
/// - `job_name_ptr`, `job_name_len`: Job name (e.g., "email_scan")
/// - `export_fn_ptr`, `export_fn_len`: WASM export function (e.g., "execute_email_scan")
/// - `resumable`: Whether the job supports resumption (1 = yes, 0 = no)
///
/// # Returns
/// 0 on success, 1 on error
pub fn host_register_job(
	mut env: FunctionEnvMut<PluginEnv>,
	job_name_ptr: WasmPtr<u8>,
	job_name_len: u32,
	export_fn_ptr: WasmPtr<u8>,
	export_fn_len: u32,
	resumable: u32,
) -> i32 {
	let (plugin_env, mut store) = env.data_and_store_mut();
	let memory = &plugin_env.memory;
	let memory_view = memory.view(&store);

	// Read job name
	let job_name = match read_string_from_wasm(&memory_view, job_name_ptr, job_name_len) {
		Ok(name) => name,
		Err(e) => {
			tracing::error!("Failed to read job name: {}", e);
			return 1; // Error
		}
	};

	// Read export function name
	let export_fn = match read_string_from_wasm(&memory_view, export_fn_ptr, export_fn_len) {
		Ok(name) => name,
		Err(e) => {
			tracing::error!("Failed to read export function name: {}", e);
			return 1; // Error
		}
	};

	let is_resumable = resumable != 0;

	// Register the job synchronously (no async needed)
	let result = plugin_env.job_registry.register(
		plugin_env.extension_id.clone(),
		job_name,
		export_fn,
		is_resumable,
	);

	match result {
		Ok(()) => 0, // Success
		Err(e) => {
			tracing::error!("Failed to register job: {}", e);
			1 // Error
		}
	}
}
