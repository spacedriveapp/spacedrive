use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindEvent {
	pub id: String,
}

/// State for managing registered keybinds
pub struct KeybindState {
	/// Maps keybind id to accelerator string
	registered: Mutex<HashMap<String, String>>,
}

impl KeybindState {
	pub fn new() -> Self {
		Self {
			registered: Mutex::new(HashMap::new()),
		}
	}
}

impl Default for KeybindState {
	fn default() -> Self {
		Self::new()
	}
}

/// Register a global keyboard shortcut
///
/// Note: This currently uses window-level keyboard shortcuts rather than
/// true global shortcuts (which require the tauri-plugin-global-shortcut).
/// For now, keybinds only work when the app window is focused.
#[tauri::command]
pub async fn register_global_keybind<R: Runtime>(
	app: AppHandle<R>,
	state: tauri::State<'_, KeybindState>,
	id: String,
	accelerator: String,
) -> Result<(), String> {
	let mut registered = state
		.registered
		.lock()
		.map_err(|e| format!("Lock error: {}", e))?;

	// Store the registration
	registered.insert(id.clone(), accelerator.clone());

	tracing::debug!("Registered keybind: {} -> {}", id, accelerator);

	// Note: For true global shortcuts, we would use tauri-plugin-global-shortcut here.
	// For now, we rely on the web listener for keyboard events.
	// The registration is stored so we can emit events when shortcuts are triggered.

	drop(registered);

	Ok(())
}

/// Unregister a global keyboard shortcut
#[tauri::command]
pub async fn unregister_global_keybind<R: Runtime>(
	_app: AppHandle<R>,
	state: tauri::State<'_, KeybindState>,
	id: String,
) -> Result<(), String> {
	let mut registered = state
		.registered
		.lock()
		.map_err(|e| format!("Lock error: {}", e))?;

	if registered.remove(&id).is_some() {
		tracing::debug!("Unregistered keybind: {}", id);
	}

	Ok(())
}

/// Emit a keybind event to the frontend
/// This can be called when a menu item with a keybind is triggered
#[allow(dead_code)]
pub fn emit_keybind_event<R: Runtime>(app: &AppHandle<R>, id: &str) -> Result<(), String> {
	app.emit("keybind-triggered", KeybindEvent { id: id.to_string() })
		.map_err(|e| format!("Failed to emit keybind event: {}", e))
}

/// Check if a keybind is registered
#[allow(dead_code)]
pub fn is_keybind_registered(state: &KeybindState, id: &str) -> bool {
	state
		.registered
		.lock()
		.map(|r| r.contains_key(id))
		.unwrap_or(false)
}

/// Get all registered keybinds
#[allow(dead_code)]
pub fn get_registered_keybinds(state: &KeybindState) -> HashMap<String, String> {
	state
		.registered
		.lock()
		.map(|r| r.clone())
		.unwrap_or_default()
}
