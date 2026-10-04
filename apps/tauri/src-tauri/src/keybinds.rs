use std::collections::HashMap;
use std::sync::Mutex;

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

	pub fn list_all(&self) -> HashMap<String, String> {
		self.registered.lock().unwrap().clone()
	}
}

impl Default for KeybindState {
	fn default() -> Self {
		Self::new()
	}
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
