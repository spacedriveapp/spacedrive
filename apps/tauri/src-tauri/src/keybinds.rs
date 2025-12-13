use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

/// Keybind registration state
/// This tracks registered keybinds for potential use in native menus
/// Actual keyboard handling is done in JavaScript for consistent behavior
pub struct KeybindState {
	/// Map of keybind ID to accelerator string
	registered: Mutex<HashMap<String, String>>,
}

impl KeybindState {
	pub fn new() -> Self {
		Self {
			registered: Mutex::new(HashMap::new()),
		}
	}

	pub fn get_accelerator(&self, id: &str) -> Option<String> {
		self.registered.lock().unwrap().get(id).cloned()
	}

	pub fn list_all(&self) -> HashMap<String, String> {
		self.registered.lock().unwrap().clone()
	}
}

impl Default for KeybindState {
	fn default() -> Self {
		Self::new()
	}
}

/// Event payload when a keybind is triggered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindEvent {
	pub id: String,
}

/// Register a keybind
/// This stores the keybind for potential use in native menus
/// Actual keyboard handling is done in JavaScript
#[tauri::command]
pub async fn register_keybind(
	state: tauri::State<'_, KeybindState>,
	id: String,
	accelerator: String,
) -> Result<(), String> {
	let mut registered = state.registered.lock().unwrap();
	registered.insert(id.clone(), accelerator.clone());

	tracing::debug!("Keybind registered: {} -> {}", id, accelerator);

	Ok(())
}

/// Unregister a keybind
#[tauri::command]
pub async fn unregister_keybind(
	state: tauri::State<'_, KeybindState>,
	id: String,
) -> Result<(), String> {
	let mut registered = state.registered.lock().unwrap();
	registered.remove(&id);

	tracing::debug!("Keybind unregistered: {}", id);

	Ok(())
}

/// Get all registered keybinds
#[tauri::command]
pub async fn get_registered_keybinds(
	state: tauri::State<'_, KeybindState>,
) -> Result<HashMap<String, String>, String> {
	Ok(state.list_all())
}

/// Emit a keybind trigger event to the frontend
/// This can be called from Rust (e.g., from native menu actions) to trigger keybind handlers
pub fn emit_keybind_triggered(app: &AppHandle, id: &str) {
	if let Err(e) = app.emit("keybind-triggered", KeybindEvent { id: id.to_string() }) {
		tracing::error!("Failed to emit keybind-triggered event: {}", e);
	}
}
