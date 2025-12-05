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
	Core,
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

	// Initialize tracing for core logs
	let _ = tracing_subscriber::fmt()
		.with_env_filter(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("sd_core=debug")),
		)
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

	// Store global state
	let _ = RUNTIME.set(rt);
	let _ = CORE.set(core);

	println!("Embedded core initialized successfully");
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

// Helper functions
// (send_response function removed - inlined into handle_core_msg)

async fn handle_json_rpc_request(request_json: String, core: &Arc<Core>) -> serde_json::Value {
	// Try parsing as batch first, then as single request
	let result: serde_json::Value = match serde_json::from_str::<Vec<JsonRpcRequest>>(&request_json) {
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

async fn process_single_request(jsonrpc_request: JsonRpcRequest, core: &Arc<Core>) -> JsonRpcResponse {
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
