//! Spacedrive Mobile Core FFI Layer
//!
//! This module provides the FFI bridge between React Native (via Expo modules)
//! and the Spacedrive core. It's adapted from the V2 iOS core implementation
//! to work with both iOS and Android.

use std::{
	ffi::{CStr, CString},
	path::PathBuf,
	sync::Arc,
};

/// Debug logging macro that only emits output in debug builds.
/// In release builds, these calls are completely eliminated by the compiler.
macro_rules! debug_log {
	($($arg:tt)*) => {
		#[cfg(debug_assertions)]
		{
			#[cfg(target_os = "android")]
			log::debug!($($arg)*);
			#[cfg(not(target_os = "android"))]
			println!($($arg)*);
		}
	};
}

/// Info logging that's always available (both debug and release).
/// Use sparingly in release - only for critical lifecycle events.
macro_rules! info_log {
	($($arg:tt)*) => {
		#[cfg(target_os = "android")]
		log::info!($($arg)*);
		#[cfg(not(target_os = "android"))]
		println!($($arg)*);
	};
}

/// Error logging that's always available.
macro_rules! error_log {
	($($arg:tt)*) => {
		#[cfg(target_os = "android")]
		log::error!($($arg)*);
		#[cfg(not(target_os = "android"))]
		eprintln!($($arg)*);
	};
}

/// Safely creates a CString by stripping any embedded null bytes.
/// This prevents panics when converting strings that may contain null bytes
/// (e.g., from file paths or error messages).
fn safe_cstring(s: impl AsRef<str>) -> CString {
	let s = s.as_ref();
	// Replace null bytes with Unicode replacement character, then strip any remaining
	let sanitized: String = s.chars().filter(|&c| c != '\0').collect();
	CString::new(sanitized).unwrap_or_else(|_| {
		// If somehow still fails, return empty string
		CString::new("").expect("Empty string should always be valid CString")
	})
}

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;

// Timeout configuration for async operations
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const LONG_RUNNING_TIMEOUT_SECS: u64 = 120;

use sd_core::{
	infra::daemon::rpc::RpcServer,
	infra::daemon::types::{DaemonError, DaemonRequest, DaemonResponse},
	infra::event::log_emitter::{set_global_log_bus, LogEventLayer},
	Core,
};

#[cfg(target_os = "android")]
use jni::{
	objects::{JClass, JObject, JString},
	sys::{jint, jstring},
	JNIEnv,
};

// Global state for embedded core
static RUNTIME: OnceCell<Runtime> = OnceCell::new();
static CORE: OnceCell<Arc<Core>> = OnceCell::new();

// JSON-RPC protocol types
#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
	jsonrpc: String,
	method: String,
	params: JsonRpcParams,
	id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcParams {
	input: serde_json::Value,
	library_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
	jsonrpc: String,
	id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	result: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcError {
	code: i32,
	message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	data: Option<JsonRpcErrorData>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcErrorData {
	/// Specific error type for client-side handling
	error_type: String,
	/// Additional details about the error
	#[serde(skip_serializing_if = "Option::is_none")]
	details: Option<serde_json::Value>,
}

/// Map DaemonError variants to JSON-RPC error codes
/// Standard JSON-RPC codes: -32700 to -32600
/// Application-specific codes: -32000 to -32099
fn daemon_error_to_jsonrpc(error: &DaemonError) -> (i32, String, JsonRpcErrorData) {
	match error {
		DaemonError::ConnectionFailed(msg) => (
			-32001,
			format!("Connection failed: {}", msg),
			JsonRpcErrorData {
				error_type: "CONNECTION_FAILED".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::ReadError(msg) => (
			-32002,
			format!("Read error: {}", msg),
			JsonRpcErrorData {
				error_type: "READ_ERROR".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::WriteError(msg) => (
			-32003,
			format!("Write error: {}", msg),
			JsonRpcErrorData {
				error_type: "WRITE_ERROR".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::RequestTooLarge(msg) => (
			-32004,
			format!("Request too large: {}", msg),
			JsonRpcErrorData {
				error_type: "REQUEST_TOO_LARGE".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::InvalidRequest(msg) => (
			-32600,
			format!("Invalid request: {}", msg),
			JsonRpcErrorData {
				error_type: "INVALID_REQUEST".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::SerializationError(msg) => (
			-32005,
			format!("Serialization error: {}", msg),
			JsonRpcErrorData {
				error_type: "SERIALIZATION_ERROR".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::DeserializationError(msg) => (
			-32006,
			format!("Deserialization error: {}", msg),
			JsonRpcErrorData {
				error_type: "DESERIALIZATION_ERROR".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::HandlerNotFound(method) => (
			-32601,
			format!("Method not found: {}", method),
			JsonRpcErrorData {
				error_type: "HANDLER_NOT_FOUND".to_string(),
				details: Some(serde_json::json!({ "method": method })),
			},
		),
		DaemonError::OperationFailed(msg) => (
			-32007,
			format!("Operation failed: {}", msg),
			JsonRpcErrorData {
				error_type: "OPERATION_FAILED".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::CoreUnavailable(msg) => (
			-32008,
			format!("Core unavailable: {}", msg),
			JsonRpcErrorData {
				error_type: "CORE_UNAVAILABLE".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::ValidationError(msg) => (
			-32009,
			format!("Validation error: {}", msg),
			JsonRpcErrorData {
				error_type: "VALIDATION_ERROR".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::SecurityError(msg) => (
			-32010,
			format!("Security error: {}", msg),
			JsonRpcErrorData {
				error_type: "SECURITY_ERROR".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
		DaemonError::InternalError(msg) => (
			-32603,
			format!("Internal error: {}", msg),
			JsonRpcErrorData {
				error_type: "INTERNAL_ERROR".to_string(),
				details: Some(serde_json::json!({ "reason": msg })),
			},
		),
	}
}

/// Initialize the embedded core with full Spacedrive functionality
///
/// # Safety
/// - `data_dir` must be a valid, non-null pointer to a null-terminated C string
/// - `device_name` may be null, but if non-null must be a valid pointer to a null-terminated C string
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn initialize_core(
	data_dir: *const std::os::raw::c_char,
	device_name: *const std::os::raw::c_char,
) -> std::os::raw::c_int {
	// Initialize Android logging first so we can see output in logcat
	#[cfg(target_os = "android")]
	{
		android_logger::init_once(
			android_logger::Config::default()
				.with_max_level(if cfg!(debug_assertions) {
					log::LevelFilter::Debug
				} else {
					log::LevelFilter::Info
				})
				.with_tag("sd-mobile-core"),
		);
		log::info!("Android logger initialized for sd-mobile-core");
	}

	// SAFETY: Validate data_dir is not null before dereferencing
	if data_dir.is_null() {
		error_log!("initialize_core: data_dir is null");
		return -2; // Error code for null pointer
	}

	let data_dir_str = unsafe { CStr::from_ptr(data_dir).to_string_lossy().to_string() };

	let device_name_opt = if device_name.is_null() {
		None
	} else {
		let name = unsafe { CStr::from_ptr(device_name).to_string_lossy().to_string() };
		if name.is_empty() {
			None
		} else {
			Some(name)
		}
	};

	// Set up panic hook to log panics
	std::panic::set_hook(Box::new(|panic_info| {
		let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
			s.to_string()
		} else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
			s.clone()
		} else {
			"Unknown panic".to_string()
		};
		let location = panic_info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown location".to_string());

		// Use Android logging for better logcat visibility
		#[cfg(target_os = "android")]
		log::error!("RUST PANIC: {} at {}", msg, location);

		#[cfg(not(target_os = "android"))]
		eprintln!("RUST PANIC: {} at {}", msg, location);
	}));

	info_log!(
		"Initializing embedded Spacedrive core with data dir: {}, device name: {:?}",
		data_dir_str, device_name_opt
	);

	// Check if already initialized (singleton pattern)
	if RUNTIME.get().is_some() && CORE.get().is_some() {
		debug_log!("Embedded core already initialized, skipping");
		return 0;
	}

	// Initialize tracing for core logs with LogEventLayer
	use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

	let _ = tracing_subscriber::registry()
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,sd_core=debug")),
		)
		.with(tracing_subscriber::fmt::layer().with_ansi(false))
		.with(LogEventLayer::new())
		.try_init();

	// Initialize Tokio runtime
	let rt = match Runtime::new() {
		Ok(rt) => rt,
		Err(e) => {
			error_log!("Failed to create Tokio runtime: {}", e);
			return -1;
		}
	};

	// Ensure data directory exists
	let data_path = PathBuf::from(data_dir_str.clone());
	if let Err(e) = std::fs::create_dir_all(&data_path) {
		error_log!("Failed to create data directory: {}", e);
		return -1;
	}

	// Initialize core
	let core =
		rt.block_on(async { Core::new_with_config(data_path, None, device_name_opt).await });

	let mut core = match core {
		Ok(core) => core,
		Err(e) => {
			error_log!("Failed to initialize core: {}", e);
			return -1;
		}
	};

	// Try to initialize networking - may fail on mobile due to platform restrictions
	// iOS: Limited background networking capabilities
	// Android: SELinux may deny access to /sys/class/net for interface enumeration
	let networking_result = rt.block_on(async {
		debug_log!("Initializing networking with protocol registration...");
		core.init_networking().await
	});

	match networking_result {
		Ok(()) => {
			info_log!("Networking initialized with protocol registration");
		}
		Err(e) => {
			error_log!("Failed to initialize networking: {}", e);
			info_log!("Continuing without networking (pairing will not work)");
			// Log more details on Android
			#[cfg(target_os = "android")]
			log::warn!("Android networking init failed: {}. Device sync will not be available.", e);
		}
	}

	let core = Arc::new(core);

	// Set global log bus for log streaming
	set_global_log_bus(core.logs.clone());

	// Store global state
	let _ = RUNTIME.set(rt);
	let _ = CORE.set(core);

	// Emit test logs
	use tracing::info;
	info!("Mobile core initialized successfully");

	0 // Success
}

/// Shutdown the embedded core
#[no_mangle]
pub extern "C" fn shutdown_core() {
	info_log!("Shutting down embedded core...");
	info_log!("Core shut down");
}

/// Handle JSON-RPC message from the embedded core
///
/// # Safety
/// - `query` must be a valid, non-null pointer to a null-terminated C string
/// - `callback` must be a valid function pointer
/// - `callback_data` is passed through to the callback and may be null
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn handle_core_msg(
	query: *const std::os::raw::c_char,
	callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char),
	callback_data: *mut std::os::raw::c_void,
) {
	// SAFETY: Validate query pointer before dereferencing
	if query.is_null() {
		let error_json = r#"{"jsonrpc":"2.0","id":"","error":{"code":-32600,"message":"Query pointer is null"}}"#;
		let error_cstring = safe_cstring(error_json);
		callback(callback_data, error_cstring.as_ptr());
		return;
	}

	let query_str = unsafe { CStr::from_ptr(query).to_string_lossy().to_string() };

	debug_log!("[RPC REQUEST]: {}", query_str);

	// Get global state
	let runtime = match RUNTIME.get() {
		Some(rt) => rt,
		None => {
			let error_json = r#"{"jsonrpc":"2.0","id":"","error":{"code":-32603,"message":"Runtime not initialized"}}"#;
			let error_cstring = safe_cstring(error_json);
			callback(callback_data, error_cstring.as_ptr());
			return;
		}
	};

	let core = match CORE.get() {
		Some(core) => core,
		None => {
			let error_json = r#"{"jsonrpc":"2.0","id":"","error":{"code":-32603,"message":"Core not initialized"}}"#;
			let error_cstring = safe_cstring(error_json);
			callback(callback_data, error_cstring.as_ptr());
			return;
		}
	};

	// Convert callback pointers to usize for Send safety
	let callback_fn_ptr: usize = callback as usize;
	let callback_data_int: usize = callback_data as usize;

	// SAFETY: Validate callback pointer is non-zero before transmute
	if callback_fn_ptr == 0 {
		error_log!("handle_core_msg: callback function pointer is null");
		return;
	}

	// Spawn async task to handle the request
	runtime.spawn(async move {
		let response = handle_json_rpc_request(query_str, core).await;
		let response_json = serde_json::to_string(&response).unwrap_or_else(|_|
			r#"{"jsonrpc":"2.0","id":"","error":{"code":-32603,"message":"Response serialization failed"}}"#.to_string()
		);

		debug_log!("[RPC RESPONSE]: {}", response_json);

		let response_cstring = safe_cstring(response_json);
		// SAFETY: callback_fn_ptr was validated as non-zero before spawning
		let callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char) =
			unsafe { std::mem::transmute(callback_fn_ptr) };
		let callback_data_ptr: *mut std::os::raw::c_void =
			callback_data_int as *mut std::os::raw::c_void;

		callback(callback_data_ptr, response_cstring.as_ptr());
	});
}

/// Start listening for core events using the real event system
#[no_mangle]
pub extern "C" fn spawn_core_event_listener(
	callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char),
	callback_data: *mut std::os::raw::c_void,
) {
	debug_log!("Starting core event listener...");

	let core = match CORE.get() {
		Some(core) => core,
		None => {
			error_log!("Core not initialized, cannot start event listener");
			return;
		}
	};

	let runtime = match RUNTIME.get() {
		Some(rt) => rt,
		None => {
			error_log!("Runtime not initialized, cannot start event listener");
			return;
		}
	};

	let callback_fn_ptr: usize = callback as usize;
	let callback_data_int: usize = callback_data as usize;

	// SAFETY: Validate callback pointer is non-zero before transmute
	if callback_fn_ptr == 0 {
		error_log!("spawn_core_event_listener: callback function pointer is null");
		return;
	}

	let mut event_subscriber = core.events.subscribe();

	runtime.spawn(async move {
		while let Ok(event) = event_subscriber.recv().await {
			let event_json = match serde_json::to_string(&event) {
				Ok(json) => json,
				Err(e) => {
					error_log!("Failed to serialize event: {}", e);
					continue;
				}
			};

			debug_log!("Broadcasting event: {}", event_json);

			let event_cstring = safe_cstring(event_json);
			// SAFETY: callback_fn_ptr was validated as non-zero before spawning
			let callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char) =
				unsafe { std::mem::transmute(callback_fn_ptr) };
			let callback_data_ptr: *mut std::os::raw::c_void =
				callback_data_int as *mut std::os::raw::c_void;
			callback(callback_data_ptr, event_cstring.as_ptr());
		}
	});
}

/// Start listening for core log messages
#[no_mangle]
pub extern "C" fn spawn_core_log_listener(
	callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char),
	callback_data: *mut std::os::raw::c_void,
) {
	debug_log!("[FFI] spawn_core_log_listener called");

	let core = match CORE.get() {
		Some(core) => core,
		None => {
			error_log!("[FFI] Core not initialized, cannot start log listener");
			return;
		}
	};

	debug_log!("[FFI] Core found, subscribing to LogBus...");
	let runtime = match RUNTIME.get() {
		Some(rt) => rt,
		None => {
			error_log!("[FFI] Runtime not initialized, cannot start log listener");
			return;
		}
	};

	let callback_fn_ptr: usize = callback as usize;
	let callback_data_int: usize = callback_data as usize;

	// SAFETY: Validate callback pointer is non-zero before transmute
	if callback_fn_ptr == 0 {
		error_log!("[FFI] spawn_core_log_listener: callback function pointer is null");
		return;
	}

	let mut log_subscriber = core.logs.subscribe();
	debug_log!(
		"[FFI] Log subscriber created, current subscriber count: {}",
		core.logs.subscriber_count()
	);

	runtime.spawn(async move {
		debug_log!("[FFI] Log listener task spawned, waiting for logs...");
		while let Ok(log) = log_subscriber.recv().await {
			let log_json = match serde_json::to_string(&log) {
				Ok(json) => json,
				Err(e) => {
					error_log!("[FFI] Failed to serialize log: {}", e);
					continue;
				}
			};

			debug_log!("[FFI] Broadcasting log: {}", log_json);

			let log_cstring = safe_cstring(log_json);
			// SAFETY: callback_fn_ptr was validated as non-zero before spawning
			let callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char) =
				unsafe { std::mem::transmute(callback_fn_ptr) };
			let callback_data_ptr: *mut std::os::raw::c_void =
				callback_data_int as *mut std::os::raw::c_void;
			callback(callback_data_ptr, log_cstring.as_ptr());
		}
		debug_log!("[FFI] Log listener task ended");
	});
}

// Helper functions
// (send_response function removed - inlined into handle_core_msg)

/// List of methods that are known to take longer and require extended timeout
const LONG_RUNNING_METHODS: &[&str] = &[
	"action:locations.add",
	"action:locations.rescan",
	"action:libraries.create",
	"action:jobs.run",
	"action:sync.full_sync",
];

/// Check if a method is long-running and requires extended timeout
fn is_long_running_method(method: &str) -> bool {
	LONG_RUNNING_METHODS
		.iter()
		.any(|&prefix| method.starts_with(prefix))
}

/// Get appropriate timeout duration for a method
fn get_timeout_for_method(method: &str) -> Duration {
	if is_long_running_method(method) {
		Duration::from_secs(LONG_RUNNING_TIMEOUT_SECS)
	} else {
		Duration::from_secs(DEFAULT_TIMEOUT_SECS)
	}
}

async fn handle_json_rpc_request(request_json: String, core: &Arc<Core>) -> serde_json::Value {
	// Try parsing as batch first, then as single request
	let result: serde_json::Value = match serde_json::from_str::<Vec<JsonRpcRequest>>(&request_json)
	{
		Ok(batch) => {
			// Handle batch of requests
			let mut responses = Vec::new();
			for req in batch {
				responses.push(process_single_request(req, core).await);
			}
			serde_json::to_value(responses).unwrap_or_else(|e| {
				serde_json::json!({
					"jsonrpc": "2.0",
					"id": "",
					"error": {
						"code": -32603,
						"message": format!("Failed to serialize batch response: {}", e)
					}
				})
			})
		}
		Err(_) => {
			// Try as single request
			match serde_json::from_str::<JsonRpcRequest>(&request_json) {
				Ok(req) => {
					let response = process_single_request(req, core).await;
					serde_json::to_value(response).unwrap_or_else(|e| {
						serde_json::json!({
							"jsonrpc": "2.0",
							"id": "",
							"error": {
								"code": -32603,
								"message": format!("Failed to serialize response: {}", e)
							}
						})
					})
				}
				Err(e) => {
					serde_json::json!({
						"jsonrpc": "2.0",
						"id": "",
						"error": {
							"code": -32700,
							"message": format!("Parse error: {}", e)
						}
					})
				}
			}
		}
	};

	result
}

async fn process_single_request(
	jsonrpc_request: JsonRpcRequest,
	core: &Arc<Core>,
) -> JsonRpcResponse {
	// Validate library_id if provided - ensure it's open before processing
	if let Some(ref lib_id_str) = jsonrpc_request.params.library_id {
		match Uuid::parse_str(lib_id_str) {
			Ok(uuid) => {
				// Check if library is open using the libraries manager
				let library = core.libraries.get_library(uuid).await;
				if library.is_none() {
					return JsonRpcResponse {
						jsonrpc: "2.0".to_string(),
						id: jsonrpc_request.id,
						result: None,
						error: Some(JsonRpcError {
							code: -32004,
							message: format!("Library not found or not open: {}", lib_id_str),
							data: Some(JsonRpcErrorData {
								error_type: "LIBRARY_NOT_FOUND".to_string(),
								details: Some(serde_json::json!({ "library_id": lib_id_str })),
							}),
						}),
					};
				}
			}
			Err(e) => {
				return JsonRpcResponse {
					jsonrpc: "2.0".to_string(),
					id: jsonrpc_request.id,
					result: None,
					error: Some(JsonRpcError {
						code: -32602,
						message: format!("Invalid library ID format: {}", e),
						data: Some(JsonRpcErrorData {
							error_type: "INVALID_LIBRARY_ID".to_string(),
							details: Some(serde_json::json!({ "library_id": lib_id_str, "reason": e.to_string() })),
						}),
					}),
				};
			}
		}
	}

	let (daemon_request, request_id) = match convert_jsonrpc_to_daemon_request(&jsonrpc_request) {
		Ok(converted) => converted,
		Err(e) => {
			return JsonRpcResponse {
				jsonrpc: "2.0".to_string(),
				id: jsonrpc_request.id,
				result: None,
				error: Some(JsonRpcError {
					code: -32601,
					message: e.clone(),
					data: Some(JsonRpcErrorData {
						error_type: "INVALID_METHOD".to_string(),
						details: Some(serde_json::json!({ "reason": e })),
					}),
				}),
			};
		}
	};

	// Determine timeout based on method type
	let timeout_duration = get_timeout_for_method(&jsonrpc_request.method);

	// Process with timeout
	let daemon_response =
		match tokio::time::timeout(timeout_duration, process_daemon_request(daemon_request, core))
			.await
		{
			Ok(response) => response,
			Err(_elapsed) => {
				let timeout_secs = timeout_duration.as_secs();
				return JsonRpcResponse {
					jsonrpc: "2.0".to_string(),
					id: request_id,
					result: None,
					error: Some(JsonRpcError {
						code: -32000,
						message: format!(
							"Request timeout after {}s: {}",
							timeout_secs, jsonrpc_request.method
						),
						data: Some(JsonRpcErrorData {
							error_type: "TIMEOUT".to_string(),
							details: Some(serde_json::json!({
								"method": jsonrpc_request.method,
								"timeout_secs": timeout_secs
							})),
						}),
					}),
				};
			}
		};

	convert_daemon_response_to_jsonrpc(daemon_response, request_id)
}

fn convert_jsonrpc_to_daemon_request(
	jsonrpc: &JsonRpcRequest,
) -> Result<(DaemonRequest, String), String> {
	let library_id = if let Some(lib_id_str) = &jsonrpc.params.library_id {
		Some(Uuid::parse_str(lib_id_str).map_err(|e| format!("Invalid library ID: {}", e))?)
	} else {
		None
	};

	let daemon_request = if jsonrpc.method.starts_with("query:") {
		let payload = if jsonrpc.params.input.is_object()
			&& jsonrpc
				.params
				.input
				.as_object()
				.map(|o| o.is_empty())
				.unwrap_or(false)
		{
			serde_json::Value::Null
		} else {
			jsonrpc.params.input.clone()
		};

		DaemonRequest::Query {
			method: jsonrpc.method.clone(),
			library_id,
			payload,
		}
	} else if jsonrpc.method.starts_with("action:") {
		DaemonRequest::Action {
			method: jsonrpc.method.clone(),
			library_id,
			payload: jsonrpc.params.input.clone(),
		}
	} else {
		return Err(format!("Invalid method prefix: {}", jsonrpc.method));
	};

	Ok((daemon_request, jsonrpc.id.clone()))
}

async fn process_daemon_request(request: DaemonRequest, core: &Arc<Core>) -> DaemonResponse {
	match request {
		DaemonRequest::Query {
			method,
			library_id,
			payload,
		} => match RpcServer::execute_json_operation(&method, library_id, payload, core).await {
			Ok(json_result) => DaemonResponse::JsonOk(json_result),
			Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
		},
		DaemonRequest::Action {
			method,
			library_id,
			payload,
		} => match RpcServer::execute_json_operation(&method, library_id, payload, core).await {
			Ok(json_result) => DaemonResponse::JsonOk(json_result),
			Err(e) => DaemonResponse::Error(DaemonError::OperationFailed(e)),
		},
		_ => DaemonResponse::Error(DaemonError::OperationFailed(
			"Unsupported request type".to_string(),
		)),
	}
}

fn convert_daemon_response_to_jsonrpc(
	response: DaemonResponse,
	request_id: String,
) -> JsonRpcResponse {
	match response {
		DaemonResponse::JsonOk(json) => JsonRpcResponse {
			jsonrpc: "2.0".to_string(),
			id: request_id,
			result: Some(json),
			error: None,
		},
		DaemonResponse::Error(daemon_error) => {
			let (code, message, data) = daemon_error_to_jsonrpc(&daemon_error);
			JsonRpcResponse {
				jsonrpc: "2.0".to_string(),
				id: request_id,
				result: None,
				error: Some(JsonRpcError {
					code,
					message,
					data: Some(data),
				}),
			}
		}
		_ => JsonRpcResponse {
			jsonrpc: "2.0".to_string(),
			id: request_id,
			result: None,
			error: Some(JsonRpcError {
				code: -32603,
				message: "Unsupported response type".to_string(),
				data: Some(JsonRpcErrorData {
					error_type: "UNSUPPORTED_RESPONSE".to_string(),
					details: None,
				}),
			}),
		},
	}
}

// Unit tests for FFI layer
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_safe_cstring_strips_nulls() {
		// Test that embedded null bytes are stripped
		let input = "hello\0world\0test";
		let result = safe_cstring(input);
		assert_eq!(result.to_str().unwrap(), "helloworldtest");
	}

	#[test]
	fn test_safe_cstring_empty_string() {
		// Test empty string handling
		let result = safe_cstring("");
		assert_eq!(result.to_str().unwrap(), "");
	}

	#[test]
	fn test_safe_cstring_normal_string() {
		// Test normal string without nulls
		let input = "normal string without nulls";
		let result = safe_cstring(input);
		assert_eq!(result.to_str().unwrap(), input);
	}

	#[test]
	fn test_safe_cstring_unicode() {
		// Test unicode handling
		let input = "hello\u{1F600}world"; // Contains emoji
		let result = safe_cstring(input);
		assert_eq!(result.to_str().unwrap(), input);
	}

	#[test]
	fn test_safe_cstring_only_nulls() {
		// Test string with only null bytes
		let input = "\0\0\0";
		let result = safe_cstring(input);
		assert_eq!(result.to_str().unwrap(), "");
	}

	#[test]
	fn test_daemon_error_connection_failed() {
		let error = DaemonError::ConnectionFailed("test connection".to_string());
		let (code, message, data) = daemon_error_to_jsonrpc(&error);
		assert_eq!(code, -32001);
		assert!(message.contains("Connection failed"));
		assert_eq!(data.error_type, "CONNECTION_FAILED");
	}

	#[test]
	fn test_daemon_error_handler_not_found() {
		let error = DaemonError::HandlerNotFound("unknownMethod".to_string());
		let (code, message, data) = daemon_error_to_jsonrpc(&error);
		assert_eq!(code, -32601); // Standard JSON-RPC method not found code
		assert!(message.contains("Method not found"));
		assert_eq!(data.error_type, "HANDLER_NOT_FOUND");
	}

	#[test]
	fn test_daemon_error_invalid_request() {
		let error = DaemonError::InvalidRequest("bad format".to_string());
		let (code, message, data) = daemon_error_to_jsonrpc(&error);
		assert_eq!(code, -32600); // Standard JSON-RPC invalid request code
		assert!(message.contains("Invalid request"));
		assert_eq!(data.error_type, "INVALID_REQUEST");
	}

	#[test]
	fn test_daemon_error_internal_error() {
		let error = DaemonError::InternalError("internal issue".to_string());
		let (code, message, data) = daemon_error_to_jsonrpc(&error);
		assert_eq!(code, -32603); // Standard JSON-RPC internal error code
		assert!(message.contains("Internal error"));
		assert_eq!(data.error_type, "INTERNAL_ERROR");
	}

	#[test]
	fn test_daemon_error_security_error() {
		let error = DaemonError::SecurityError("unauthorized".to_string());
		let (code, message, data) = daemon_error_to_jsonrpc(&error);
		assert_eq!(code, -32010);
		assert!(message.contains("Security error"));
		assert_eq!(data.error_type, "SECURITY_ERROR");
	}

	#[test]
	fn test_daemon_error_validation_error() {
		let error = DaemonError::ValidationError("invalid input".to_string());
		let (code, message, data) = daemon_error_to_jsonrpc(&error);
		assert_eq!(code, -32009);
		assert!(message.contains("Validation error"));
		assert_eq!(data.error_type, "VALIDATION_ERROR");
	}

	#[test]
	fn test_daemon_error_core_unavailable() {
		let error = DaemonError::CoreUnavailable("shutting down".to_string());
		let (code, message, data) = daemon_error_to_jsonrpc(&error);
		assert_eq!(code, -32008);
		assert!(message.contains("Core unavailable"));
		assert_eq!(data.error_type, "CORE_UNAVAILABLE");
	}
}

// Android JNI bindings
#[cfg(target_os = "android")]
mod android {
	use super::*;
	use jni::{
		objects::{GlobalRef, JClass, JObject, JString, JValue},
		sys::{jint, jstring},
		JNIEnv, JavaVM,
	};
	use once_cell::sync::OnceCell;
	use std::sync::Arc;

	static JAVA_VM: OnceCell<Arc<JavaVM>> = OnceCell::new();
	static EVENT_MODULE_REF: OnceCell<GlobalRef> = OnceCell::new();
	static LOG_MODULE_REF: OnceCell<GlobalRef> = OnceCell::new();

	/// Helper function to safely reject a promise with an error message.
	/// Returns Ok(()) if the rejection succeeded, Err with the failure reason otherwise.
	fn reject_promise(env: &mut JNIEnv, promise: &GlobalRef, error: &str) {
		let result = (|| -> Result<(), String> {
			let error_jstring = env.new_string(error).map_err(|e| format!("Failed to create error string: {}", e))?;
			env.call_method(
				promise.as_obj(),
				"reject",
				"(Ljava/lang/String;)V",
				&[JValue::Object(&error_jstring)],
			).map_err(|e| format!("Failed to call reject method: {}", e))?;
			Ok(())
		})();

		if let Err(e) = result {
			log::error!("Failed to reject promise: {}", e);
		}
	}

	// Only for Android x86_64 - provides missing symbol
	#[cfg(all(target_os = "android", target_arch = "x86_64"))]
	#[no_mangle]
	pub extern "C" fn __rust_probestack() {
		// Intentionally empty - stack probing disabled for x86_64 emulator
	}

	#[no_mangle]
	pub unsafe extern "C" fn Java_com_spacedrive_core_SDMobileCoreModule_initializeCore(
		mut env: JNIEnv,
		_class: JClass,
		data_dir: JString,
		device_name: JString,
	) -> jint {
		let data_dir_str: String = env
			.get_string(&data_dir)
			.expect("Failed to get data_dir string")
			.into();

		let device_name_str = if device_name.is_null() {
			None
		} else {
			Some(
				env.get_string(&device_name)
					.expect("Failed to get device_name string")
					.into(),
			)
		};

		let data_dir_cstr = safe_cstring(data_dir_str);
		let device_name_cstr = device_name_str.map(|s: String| safe_cstring(s));

		let result = super::initialize_core(
			data_dir_cstr.as_ptr(),
			device_name_cstr
				.as_ref()
				.map(|s| s.as_ptr())
				.unwrap_or(std::ptr::null()),
		);

		result as jint
	}

	#[no_mangle]
	pub unsafe extern "C" fn Java_com_spacedrive_core_SDMobileCoreModule_shutdownCore(
		_env: JNIEnv,
		_class: JClass,
	) {
		super::shutdown_core();
	}

	#[no_mangle]
	pub unsafe extern "C" fn Java_com_spacedrive_core_SDMobileCoreModule_handleCoreMsg(
		mut env: JNIEnv,
		_class: JClass,
		query: JString,
		promise: JObject,
	) {
		// CRITICAL: Capture JavaVM before spawning async task
		// The async callback will run on a Tokio worker thread that needs JVM access
		if JAVA_VM.get().is_none() {
			if let Ok(jvm) = env.get_java_vm() {
				let _ = JAVA_VM.set(Arc::new(jvm));
			}
		}

		let query_str: String = env
			.get_string(&query)
			.expect("Failed to get query string")
			.into();
		let query_cstr = safe_cstring(query_str);

		let promise_ref = env.new_global_ref(promise).unwrap();

		extern "C" fn android_callback(
			data: *mut std::os::raw::c_void,
			result: *const std::os::raw::c_char,
		) {
			// Wrap entire callback in catch_unwind to prevent panics from crossing FFI boundary
			let callback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				// SAFETY: Validate data pointer before Box::from_raw
				if data.is_null() {
					log::error!("android_callback: data pointer is null");
					return;
				}
				// SAFETY: Validate result pointer before CStr::from_ptr
				if result.is_null() {
					log::error!("android_callback: result pointer is null");
					return;
				}

				let promise_ref = unsafe { Box::from_raw(data as *mut GlobalRef) };
				let result_str = unsafe { CStr::from_ptr(result).to_string_lossy().to_string() };

				let jvm = match JAVA_VM.get() {
					Some(jvm) => jvm,
					None => {
						log::error!("android_callback: JavaVM not initialized");
						return;
					}
				};

				let mut env = match jvm.attach_current_thread() {
					Ok(env) => env,
					Err(e) => {
						log::error!("android_callback: Failed to attach thread: {}", e);
						return;
					}
				};

				let result_jstring = match env.new_string(&result_str) {
					Ok(s) => s,
					Err(e) => {
						log::error!("android_callback: Failed to create result string: {}", e);
						reject_promise(&mut env, &promise_ref, &format!("JNI error: {}", e));
						return;
					}
				};

				if let Err(e) = env.call_method(
					promise_ref.as_obj(),
					"resolve",
					"(Ljava/lang/String;)V",
					&[JValue::Object(&result_jstring)],
				) {
					log::error!("android_callback: Failed to resolve promise: {}", e);
				}
			}));

			if let Err(e) = callback_result {
				log::error!("android_callback: Panic caught: {:?}", e);
			}
		}

		let promise_ptr = Box::into_raw(Box::new(promise_ref)) as *mut std::os::raw::c_void;

		super::handle_core_msg(query_cstr.as_ptr(), android_callback, promise_ptr);
	}

	#[no_mangle]
	pub unsafe extern "C" fn Java_com_spacedrive_core_SDMobileCoreModule_registerCoreEventListener(
		mut env: JNIEnv,
		module: JObject,
	) {
		let jvm = env.get_java_vm().unwrap();
		let _ = JAVA_VM.set(Arc::new(jvm));

		let module_ref = env.new_global_ref(module).unwrap();
		let _ = EVENT_MODULE_REF.set(module_ref);

		extern "C" fn android_event_callback(
			_data: *mut std::os::raw::c_void,
			event: *const std::os::raw::c_char,
		) {
			// Wrap entire callback in catch_unwind to prevent panics from crossing FFI boundary
			let callback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				let event_str = unsafe { CStr::from_ptr(event).to_string_lossy().to_string() };

				let jvm = match JAVA_VM.get() {
					Some(jvm) => jvm,
					None => {
						log::error!("android_event_callback: JavaVM not initialized");
						return;
					}
				};

				let mut env = match jvm.attach_current_thread() {
					Ok(env) => env,
					Err(e) => {
						log::error!("android_event_callback: Failed to attach thread: {}", e);
						return;
					}
				};

				let module_ref = match EVENT_MODULE_REF.get() {
					Some(r) => r,
					None => {
						log::error!("android_event_callback: Event module not initialized");
						return;
					}
				};

				let event_jstring = match env.new_string(&event_str) {
					Ok(s) => s,
					Err(e) => {
						log::error!("android_event_callback: Failed to create event string: {}", e);
						return;
					}
				};

				if let Err(e) = env.call_method(
					module_ref.as_obj(),
					"sendCoreEvent",
					"(Ljava/lang/String;)V",
					&[JValue::Object(&event_jstring)],
				) {
					log::error!("android_event_callback: Failed to send event: {}", e);
				}
			}));

			if let Err(e) = callback_result {
				log::error!("android_event_callback: Panic caught: {:?}", e);
			}
		}

		super::spawn_core_event_listener(android_event_callback, std::ptr::null_mut());
	}

	#[no_mangle]
	pub unsafe extern "C" fn Java_com_spacedrive_core_SDMobileCoreModule_registerCoreLogListener(
		mut env: JNIEnv,
		module: JObject,
	) {
		let jvm = env.get_java_vm().unwrap();
		let _ = JAVA_VM.set(Arc::new(jvm));

		let module_ref = env.new_global_ref(module).unwrap();
		let _ = LOG_MODULE_REF.set(module_ref);

		extern "C" fn android_log_callback(
			_data: *mut std::os::raw::c_void,
			log: *const std::os::raw::c_char,
		) {
			// Wrap entire callback in catch_unwind to prevent panics from crossing FFI boundary
			let callback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				let log_str = unsafe { CStr::from_ptr(log).to_string_lossy().to_string() };

				let jvm = match JAVA_VM.get() {
					Some(jvm) => jvm,
					None => {
						// Can't log this since we're in the log callback - just return
						return;
					}
				};

				let mut env = match jvm.attach_current_thread() {
					Ok(env) => env,
					Err(_) => {
						// Can't log this since we're in the log callback - just return
						return;
					}
				};

				let module_ref = match LOG_MODULE_REF.get() {
					Some(r) => r,
					None => {
						// Log module not initialized - just return
						return;
					}
				};

				let log_jstring = match env.new_string(&log_str) {
					Ok(s) => s,
					Err(_) => {
						// Failed to create string - just return
						return;
					}
				};

				// Ignore errors in log callback to avoid infinite recursion
				let _ = env.call_method(
					module_ref.as_obj(),
					"sendCoreLog",
					"(Ljava/lang/String;)V",
					&[JValue::Object(&log_jstring)],
				);
			}));

			// Silently ignore panics in log callback to avoid cascading failures
			let _ = callback_result;
		}

		super::spawn_core_log_listener(android_log_callback, std::ptr::null_mut());
	}
}
