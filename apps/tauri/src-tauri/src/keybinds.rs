//! Keybind Module
//!
//! Provides global keyboard shortcut registration and management.
//! Uses Tauri's global shortcut API to register shortcuts that work
//! even when the window is not focused.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Event payload sent to the frontend when a keybind is triggered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindEvent {
	pub id: String,
}

/// State for tracking registered keybinds
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
}

impl Default for KeybindState {
	fn default() -> Self {
		Self::new()
	}
}

/// Register a global keyboard shortcut
///
/// # Arguments
/// * `app` - The Tauri application handle
/// * `state` - The keybind state for tracking registrations
/// * `id` - Unique identifier for this keybind
/// * `accelerator` - The key combination (e.g., "Cmd+C", "Ctrl+Shift+P")
#[tauri::command]
pub async fn register_global_shortcut<R: Runtime>(
	app: AppHandle<R>,
	state: tauri::State<'_, KeybindState>,
	id: String,
	accelerator: String,
) -> Result<(), String> {
	let mut registered = state
		.registered
		.lock()
		.map_err(|e| format!("Failed to lock state: {}", e))?;

	// Unregister old shortcut if it exists with a different accelerator
	if let Some(old_accelerator) = registered.get(&id) {
		if old_accelerator != &accelerator {
			// Try to unregister the old shortcut
			if let Ok(shortcut) = old_accelerator.parse::<Shortcut>() {
				let _ = app.global_shortcut().unregister(shortcut);
			}
		} else {
			// Same accelerator, nothing to do
			return Ok(());
		}
	}

	// Parse the accelerator string
	let shortcut: Shortcut = accelerator
		.parse()
		.map_err(|e| format!("Invalid accelerator '{}': {:?}", accelerator, e))?;

	// Clone values for the closure
	let id_for_handler = id.clone();
	let app_for_handler = app.clone();

	// Register the global shortcut
	app.global_shortcut()
		.on_shortcut(shortcut, move |_app, _shortcut, event| {
			if event.state == ShortcutState::Pressed {
				// Emit event to frontend
				if let Err(e) = app_for_handler.emit(
					"keybind-triggered",
					KeybindEvent {
						id: id_for_handler.clone(),
					},
				) {
					tracing::error!("Failed to emit keybind event: {}", e);
				}
			}
		})
		.map_err(|e| format!("Failed to register shortcut: {}", e))?;

	// Track the registration
	registered.insert(id, accelerator);

	Ok(())
}

/// Unregister a global keyboard shortcut
///
/// # Arguments
/// * `app` - The Tauri application handle
/// * `state` - The keybind state for tracking registrations
/// * `id` - The keybind ID to unregister
#[tauri::command]
pub async fn unregister_global_shortcut<R: Runtime>(
	app: AppHandle<R>,
	state: tauri::State<'_, KeybindState>,
	id: String,
) -> Result<(), String> {
	let mut registered = state
		.registered
		.lock()
		.map_err(|e| format!("Failed to lock state: {}", e))?;

	if let Some(accelerator) = registered.remove(&id) {
		// Parse and unregister the shortcut
		if let Ok(shortcut) = accelerator.parse::<Shortcut>() {
			app.global_shortcut()
				.unregister(shortcut)
				.map_err(|e| format!("Failed to unregister shortcut: {}", e))?;
		}
	}

	Ok(())
}

/// Unregister all global keyboard shortcuts
/// Called during cleanup
#[allow(dead_code)]
pub fn unregister_all<R: Runtime>(app: &AppHandle<R>, state: &KeybindState) {
	if let Ok(mut registered) = state.registered.lock() {
		for (_id, accelerator) in registered.drain() {
			if let Ok(shortcut) = accelerator.parse::<Shortcut>() {
				let _ = app.global_shortcut().unregister(shortcut);
			}
		}
	}
}
