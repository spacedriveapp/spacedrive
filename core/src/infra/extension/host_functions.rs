//! WASM host functions
//!
//! The functions a guest imports from the `spacedrive` module. `spacedrive_log`
//! and `register_job` serve `plugin_init`; the `job_*` functions report into
//! the running job; `spacedrive_op` carries every operation that returns data.
//!
//! An operation is answered on the job's async side: the host function reads
//! the request out of guest memory, sends it through the `JobBridge`, blocks
//! the guest thread until the answer arrives, and writes the answer into
//! memory taken from the guest's own `wasm_alloc`. The guest frees it. One
//! convention for every operation keeps the import table small and the memory
//! handling in one place on each side.

use std::sync::Arc;

use wasmer::{FunctionEnvMut, Memory, MemoryView, TypedFunction, WasmPtr};

/// Environment passed to all host functions
pub struct PluginEnv {
	pub extension_id: String,
	pub memory: Memory,
	/// The guest's allocator, for answers. Set once the instance exists.
	pub alloc: Option<TypedFunction<i32, i32>>,
	pub job_registry: Arc<super::job_registry::ExtensionJobRegistry>,
	pub model_registry: Arc<super::model_registry::ExtensionModelRegistry>,
	pub current_job: Option<JobBridge>,
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

/// What a running job's host calls report back to its `WasmJob`.
///
/// The guest runs on a blocking thread while the job's async side drains these
/// events, so checkpoints reach the job database before the guest continues.
pub enum JobEvent {
	Progress {
		fraction: f32,
		message: String,
	},
	Checkpoint {
		state: String,
		saved: tokio::sync::oneshot::Sender<bool>,
	},
	Warning(String),
	Items(u64),
	Bytes(u64),
	/// A request for data, answered by `super::ops` on the async side.
	Op {
		op: String,
		payload: Vec<u8>,
		reply: tokio::sync::oneshot::Sender<super::ops::OpResult>,
	},
}

/// The job a plugin instance is running right now, if any.
///
/// Host functions read it instead of trusting the job id the guest passes, so a
/// guest cannot report into another job.
pub struct JobBridge {
	pub events: tokio::sync::mpsc::UnboundedSender<JobEvent>,
	pub interrupted: Arc<std::sync::atomic::AtomicBool>,
}

fn with_job<T>(env: &PluginEnv, fallback: T, f: impl FnOnce(&JobBridge) -> T) -> T {
	match &env.current_job {
		Some(bridge) => f(bridge),
		None => {
			tracing::warn!(
				extension = %env.extension_id,
				"Job host function called outside a job"
			);
			fallback
		}
	}
}

/// Report job progress
pub fn host_job_report_progress(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
	progress: f32,
	message_ptr: WasmPtr<u8>,
	message_len: u32,
) {
	let (plugin_env, store) = env.data_and_store_mut();
	let memory_view = plugin_env.memory.view(&store);

	let message = match read_string_from_wasm(&memory_view, message_ptr, message_len) {
		Ok(msg) => msg,
		Err(e) => {
			tracing::error!("Failed to read message: {}", e);
			return;
		}
	};

	with_job(plugin_env, (), |job| {
		let _ = job.events.send(JobEvent::Progress {
			fraction: progress,
			message,
		});
	});
}

/// Save job checkpoint
///
/// Blocks the guest until the state is in the job database, which is what makes
/// a checkpoint worth anything after a kill.
pub fn host_job_checkpoint(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
	state_ptr: WasmPtr<u8>,
	state_len: u32,
) -> i32 {
	let (plugin_env, store) = env.data_and_store_mut();
	let memory_view = plugin_env.memory.view(&store);

	let state = match read_string_from_wasm(&memory_view, state_ptr, state_len) {
		Ok(state) => state,
		Err(e) => {
			tracing::error!("Failed to read checkpoint state: {}", e);
			return 1;
		}
	};

	with_job(plugin_env, 1, |job| {
		let (saved_tx, saved_rx) = tokio::sync::oneshot::channel();
		if job
			.events
			.send(JobEvent::Checkpoint {
				state,
				saved: saved_tx,
			})
			.is_err()
		{
			return 1;
		}
		match saved_rx.blocking_recv() {
			Ok(true) => 0,
			_ => 1,
		}
	})
}

/// Run one data-returning operation for the job.
///
/// Writes `[ptr, len]` of the answer to `out_ptr` and returns 0, or writes a
/// JSON error there and returns 1. Both buffers come from the guest's
/// `wasm_alloc`, which the guest frees after reading. Called outside a job,
/// or with an unreadable request, nothing is written and 2 is returned.
pub fn host_spacedrive_op(
	mut env: FunctionEnvMut<PluginEnv>,
	op_ptr: WasmPtr<u8>,
	op_len: u32,
	payload_ptr: WasmPtr<u8>,
	payload_len: u32,
	out_ptr: WasmPtr<u32>,
) -> i32 {
	let (plugin_env, mut store) = env.data_and_store_mut();
	let memory = plugin_env.memory.clone();
	let memory_view = memory.view(&store);

	let op = match read_string_from_wasm(&memory_view, op_ptr, op_len) {
		Ok(op) => op,
		Err(e) => {
			tracing::error!("Failed to read op name: {}", e);
			return 2;
		}
	};
	let payload = match payload_ptr
		.slice(&memory_view, payload_len)
		.and_then(|slice| slice.read_to_vec())
	{
		Ok(payload) => payload,
		Err(e) => {
			tracing::error!("Failed to read op payload: {:?}", e);
			return 2;
		}
	};
	let Some(alloc) = plugin_env.alloc.clone() else {
		return 2;
	};

	let answer = with_job(plugin_env, None, |job| {
		let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
		if job
			.events
			.send(JobEvent::Op {
				op: op.clone(),
				payload,
				reply: reply_tx,
			})
			.is_err()
		{
			return None;
		}
		reply_rx.blocking_recv().ok()
	});
	let Some(answer) = answer else {
		return 2;
	};

	let (status, bytes) = match answer {
		Ok(bytes) => (0, bytes),
		Err(error) => (1, error.to_json().into_bytes()),
	};

	// The guest frees `len.max(1)` bytes, matching the allocation here.
	let len = bytes.len() as u32;
	let ptr = match alloc.call(&mut store, len.max(1) as i32) {
		Ok(ptr) => ptr as u32,
		Err(e) => {
			tracing::error!(extension = %plugin_env.extension_id, op, "wasm_alloc failed: {e}");
			return 2;
		}
	};
	let memory_view = memory.view(&store);
	if let Err(e) = memory_view.write(ptr as u64, &bytes) {
		tracing::error!(extension = %plugin_env.extension_id, op, "write answer: {e}");
		return 2;
	}
	if let Err(e) = out_ptr
		.slice(&memory_view, 2)
		.and_then(|slice| slice.write_slice(&[ptr, len]))
	{
		tracing::error!(extension = %plugin_env.extension_id, op, "write out pointer: {e:?}");
		return 2;
	}
	status
}

/// Fill guest memory with entropy.
///
/// Every extension gets this without a grant: random ids and the time are
/// not library data.
pub fn host_spacedrive_random(mut env: FunctionEnvMut<PluginEnv>, buf_ptr: WasmPtr<u8>, len: u32) {
	use rand::RngCore;

	let (plugin_env, store) = env.data_and_store_mut();
	let view = plugin_env.memory.view(&store);
	// Resolve the guest slice before allocating, so a bogus length is an
	// error for the guest rather than a host allocation of its choosing.
	let slice = match buf_ptr.slice(&view, len) {
		Ok(slice) => slice,
		Err(e) => {
			tracing::error!(extension = %plugin_env.extension_id, "write entropy: {e:?}");
			return;
		}
	};
	let mut bytes = vec![0u8; len as usize];
	rand::thread_rng().fill_bytes(&mut bytes);
	if let Err(e) = slice.write_slice(&bytes) {
		tracing::error!(extension = %plugin_env.extension_id, "write entropy: {e:?}");
	}
}

/// The host's clock, as milliseconds since the Unix epoch.
pub fn host_spacedrive_now_ms(_env: FunctionEnvMut<PluginEnv>) -> i64 {
	chrono::Utc::now().timestamp_millis()
}

/// Check if job should be interrupted
pub fn host_job_check_interrupt(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
) -> i32 {
	let (plugin_env, _store) = env.data_and_store_mut();
	with_job(plugin_env, 0, |job| {
		job.interrupted.load(std::sync::atomic::Ordering::SeqCst) as i32
	})
}

/// Add a warning to the job
pub fn host_job_add_warning(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
	message_ptr: WasmPtr<u8>,
	message_len: u32,
) {
	let (plugin_env, store) = env.data_and_store_mut();
	let memory_view = plugin_env.memory.view(&store);

	let message = match read_string_from_wasm(&memory_view, message_ptr, message_len) {
		Ok(msg) => msg,
		Err(e) => {
			tracing::error!("Failed to read warning: {}", e);
			return;
		}
	};

	with_job(plugin_env, (), |job| {
		let _ = job.events.send(JobEvent::Warning(message));
	});
}

/// Increment bytes processed
pub fn host_job_increment_bytes(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
	bytes: u64,
) {
	let (plugin_env, _store) = env.data_and_store_mut();
	with_job(plugin_env, (), |job| {
		let _ = job.events.send(JobEvent::Bytes(bytes));
	});
}

/// Increment items processed
pub fn host_job_increment_items(
	mut env: FunctionEnvMut<PluginEnv>,
	_job_id_ptr: WasmPtr<u8>,
	count: u64,
) {
	let (plugin_env, _store) = env.data_and_store_mut();
	with_job(plugin_env, (), |job| {
		let _ = job.events.send(JobEvent::Items(count));
	});
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

/// Declare a data model's facet, from `plugin_init`.
///
/// The definition is the JSON the SDK's `#[model]` macro derived from the
/// struct. Returns 0 on success, 1 on error.
pub fn host_register_model(
	mut env: FunctionEnvMut<PluginEnv>,
	def_ptr: WasmPtr<u8>,
	def_len: u32,
) -> i32 {
	let (plugin_env, store) = env.data_and_store_mut();
	let memory_view = plugin_env.memory.view(&store);
	let definition = match read_string_from_wasm(&memory_view, def_ptr, def_len)
		.map_err(|e| e.to_string())
		.and_then(|json| serde_json::from_str(&json).map_err(|e| e.to_string()))
	{
		Ok(definition) => definition,
		Err(e) => {
			tracing::error!(extension = %plugin_env.extension_id, "Unreadable model definition: {e}");
			return 1;
		}
	};
	match plugin_env
		.model_registry
		.register(&plugin_env.extension_id, definition)
	{
		Ok(()) => 0,
		Err(e) => {
			tracing::error!(extension = %plugin_env.extension_id, "Failed to register model: {e}");
			1
		}
	}
}
