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

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tokio::runtime::Runtime;
use uuid::Uuid;

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
}

/// Initialize the embedded core with full Spacedrive functionality
#[no_mangle]
pub extern "C" fn initialize_core(
	data_dir: *const std::os::raw::c_char,
	device_name: *const std::os::raw::c_char,
) -> std::os::raw::c_int {
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

	println!(
		"Initializing embedded Spacedrive core with data dir: {}, device name: {:?}",
		data_dir_str, device_name_opt
	);

	// Check if already initialized (singleton pattern)
	if RUNTIME.get().is_some() && CORE.get().is_some() {
		println!("Embedded core already initialized, skipping");
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
			println!("Failed to create Tokio runtime: {}", e);
			return -1;
		}
	};

	// Ensure data directory exists
	let data_path = PathBuf::from(data_dir_str.clone());
	if let Err(e) = std::fs::create_dir_all(&data_path) {
		println!("Failed to create data directory: {}", e);
		return -1;
	}

	// Initialize core
	let mut core =
		rt.block_on(async { Core::new_with_config(data_path, None, device_name_opt).await });

	let mut core = match core {
		Ok(core) => core,
		Err(e) => {
			println!("Failed to initialize core: {}", e);
			return -1;
		}
	};

	// Initialize networking with protocol registration
	let networking_result = rt.block_on(async {
		println!("Initializing networking with protocol registration...");
		core.init_networking().await
	});

	match networking_result {
		Ok(()) => {
			println!("Networking initialized with protocol registration");
		}
		Err(e) => {
			println!("Failed to initialize networking: {}", e);
			println!("Continuing without networking (pairing will not work)");
		}
	}

	let core = Arc::new(core);

	// Set global log bus for log streaming
	set_global_log_bus(core.logs.clone());

	// Store global state
	let _ = RUNTIME.set(rt);
	let _ = CORE.set(core);

	// Emit test logs
	use tracing::{error, info, warn};
	info!("Mobile core initialized successfully");

	0 // Success
}

/// Shutdown the embedded core
#[no_mangle]
pub extern "C" fn shutdown_core() {
	println!("Shutting down embedded core...");
	println!("Core shut down");
}

/// Handle JSON-RPC message from the embedded core
#[no_mangle]
pub extern "C" fn handle_core_msg(
	query: *const std::os::raw::c_char,
	callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char),
	callback_data: *mut std::os::raw::c_void,
) {
	let query_str = unsafe { CStr::from_ptr(query).to_string_lossy().to_string() };

	println!("[RPC REQUEST]: {}", query_str);

	// Get global state
	let runtime = match RUNTIME.get() {
		Some(rt) => rt,
		None => {
			let error_json = r#"{"jsonrpc":"2.0","id":"","error":{"code":-32603,"message":"Core not initialized"}}"#;
			let error_cstring = CString::new(error_json).unwrap();
			callback(callback_data, error_cstring.as_ptr());
			return;
		}
	};

	let core = CORE.get().unwrap();

	// Convert callback pointers to usize for Send safety
	let callback_fn_ptr: usize = callback as usize;
	let callback_data_int: usize = callback_data as usize;

	// Spawn async task to handle the request
	runtime.spawn(async move {
		let response = handle_json_rpc_request(query_str, core).await;
		let response_json = serde_json::to_string(&response).unwrap_or_else(|_|
			r#"{"jsonrpc":"2.0","id":"","error":{"code":-32603,"message":"Response serialization failed"}}"#.to_string()
		);

		println!("[RPC RESPONSE]: {}", response_json);

		let response_cstring = CString::new(response_json).unwrap();
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
	println!("Starting core event listener...");

	let core = match CORE.get() {
		Some(core) => core,
		None => {
			println!("Core not initialized, cannot start event listener");
			return;
		}
	};

	let runtime = RUNTIME.get().unwrap();

	let callback_fn_ptr: usize = callback as usize;
	let callback_data_int: usize = callback_data as usize;

	let mut event_subscriber = core.events.subscribe();

	runtime.spawn(async move {
		while let Ok(event) = event_subscriber.recv().await {
			let event_json = match serde_json::to_string(&event) {
				Ok(json) => json,
				Err(e) => {
					println!("Failed to serialize event: {}", e);
					continue;
				}
			};

			println!("Broadcasting event: {}", event_json);

			let event_cstring = CString::new(event_json).unwrap();
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
	println!("[FFI] spawn_core_log_listener called");

	let core = match CORE.get() {
		Some(core) => core,
		None => {
			println!("❌ [FFI] Core not initialized, cannot start log listener");
			return;
		}
	};

	println!("[FFI] Core found, subscribing to LogBus...");
	let runtime = RUNTIME.get().unwrap();

	let callback_fn_ptr: usize = callback as usize;
	let callback_data_int: usize = callback_data as usize;

	let mut log_subscriber = core.logs.subscribe();
	println!(
		"[FFI] Log subscriber created, current subscriber count: {}",
		core.logs.subscriber_count()
	);

	runtime.spawn(async move {
		println!("[FFI] Log listener task spawned, waiting for logs...");
		while let Ok(log) = log_subscriber.recv().await {
			let log_json = match serde_json::to_string(&log) {
				Ok(json) => json,
				Err(e) => {
					println!("❌ [FFI] Failed to serialize log: {}", e);
					continue;
				}
			};

			println!("[FFI] Broadcasting log: {}", log_json);

			let log_cstring = CString::new(log_json).unwrap();
			let callback: extern "C" fn(*mut std::os::raw::c_void, *const std::os::raw::c_char) =
				unsafe { std::mem::transmute(callback_fn_ptr) };
			let callback_data_ptr: *mut std::os::raw::c_void =
				callback_data_int as *mut std::os::raw::c_void;
			callback(callback_data_ptr, log_cstring.as_ptr());
		}
		println!("❌ [FFI] Log listener task ended");
	});
}

// Helper functions
// (send_response function removed - inlined into handle_core_msg)

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
			serde_json::to_value(responses).unwrap()
		}
		Err(_) => {
			// Try as single request
			match serde_json::from_str::<JsonRpcRequest>(&request_json) {
				Ok(req) => {
					let response = process_single_request(req, core).await;
					serde_json::to_value(response).unwrap()
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
	let (daemon_request, request_id) = match convert_jsonrpc_to_daemon_request(&jsonrpc_request) {
		Ok(converted) => converted,
		Err(e) => {
			return JsonRpcResponse {
				jsonrpc: "2.0".to_string(),
				id: jsonrpc_request.id,
				result: None,
				error: Some(JsonRpcError {
					code: -32601,
					message: e,
				}),
			};
		}
	};

	let daemon_response = process_daemon_request(daemon_request, core).await;

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
			&& jsonrpc.params.input.as_object().unwrap().is_empty()
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
		DaemonResponse::Error(daemon_error) => JsonRpcResponse {
			jsonrpc: "2.0".to_string(),
			id: request_id,
			result: None,
			error: Some(JsonRpcError {
				code: -32603,
				message: daemon_error.to_string(),
			}),
		},
		_ => JsonRpcResponse {
			jsonrpc: "2.0".to_string(),
			id: request_id,
			result: None,
			error: Some(JsonRpcError {
				code: -32603,
				message: "Unsupported response type".to_string(),
			}),
		},
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

		let data_dir_cstr = CString::new(data_dir_str).unwrap();
		let device_name_cstr = device_name_str.map(|s: String| CString::new(s).unwrap());

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
		let query_str: String = env
			.get_string(&query)
			.expect("Failed to get query string")
			.into();
		let query_cstr = CString::new(query_str).unwrap();

		let promise_ref = env.new_global_ref(promise).unwrap();

		extern "C" fn android_callback(
			data: *mut std::os::raw::c_void,
			result: *const std::os::raw::c_char,
		) {
			let promise_ref = unsafe { Box::from_raw(data as *mut GlobalRef) };
			let result_str = unsafe { CStr::from_ptr(result).to_string_lossy().to_string() };

			let jvm = JAVA_VM.get().expect("JavaVM not initialized");
			let mut env = jvm.attach_current_thread().unwrap();

			let result_jstring = env.new_string(&result_str).unwrap();
			env.call_method(
				promise_ref.as_obj(),
				"resolve",
				"(Ljava/lang/String;)V",
				&[JValue::Object(&result_jstring)],
			)
			.unwrap();
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
			let event_str = unsafe { CStr::from_ptr(event).to_string_lossy().to_string() };

			let jvm = JAVA_VM.get().expect("JavaVM not initialized");
			let mut env = jvm.attach_current_thread().unwrap();

			let module_ref = EVENT_MODULE_REF
				.get()
				.expect("Event module not initialized");
			let event_jstring = env.new_string(&event_str).unwrap();

			env.call_method(
				module_ref.as_obj(),
				"sendCoreEvent",
				"(Ljava/lang/String;)V",
				&[JValue::Object(&event_jstring)],
			)
			.unwrap();
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
			let log_str = unsafe { CStr::from_ptr(log).to_string_lossy().to_string() };

			let jvm = JAVA_VM.get().expect("JavaVM not initialized");
			let mut env = jvm.attach_current_thread().unwrap();

			let module_ref = LOG_MODULE_REF.get().expect("Log module not initialized");
			let log_jstring = env.new_string(&log_str).unwrap();

			env.call_method(
				module_ref.as_obj(),
				"sendCoreLog",
				"(Ljava/lang/String;)V",
				&[JValue::Object(&log_jstring)],
			)
			.unwrap();
		}

		super::spawn_core_log_listener(android_log_callback, std::ptr::null_mut());
	}
}
