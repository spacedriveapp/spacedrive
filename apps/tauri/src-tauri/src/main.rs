// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod drag;
mod files;
mod server;
mod windows;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::menu::MenuItem;
use tauri::Emitter;
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Default event subscription list - mirrors packages/ts-client/src/event-filter.ts
/// Excludes noisy events: LogMessage, JobProgress, IndexingProgress
fn get_default_event_subscription() -> Vec<&'static str> {
	vec![
		// Core lifecycle
		"CoreStarted",
		"CoreShutdown",
		// Library events
		"LibraryCreated",
		"LibraryOpened",
		"LibraryClosed",
		"LibraryDeleted",
		"LibraryStatisticsUpdated",
		// Entry events
		"EntryCreated",
		"EntryModified",
		"EntryDeleted",
		"EntryMoved",
		// Raw filesystem changes
		"FsRawChange",
		// Volume events
		"VolumeAdded",
		"VolumeRemoved",
		"VolumeUpdated",
		"VolumeSpeedTested",
		"VolumeMountChanged",
		"VolumeError",
		// Job lifecycle
		"JobQueued",
		"JobStarted",
		"JobProgress",
		"JobCompleted",
		"JobFailed",
		"JobCancelled",
		"JobPaused",
		"JobResumed",
		// Indexing lifecycle (no progress spam)
		"IndexingStarted",
		"IndexingCompleted",
		"IndexingFailed",
		// Device events
		"DeviceConnected",
		"DeviceDisconnected",
		// Resource events
		"ResourceChanged",
		"ResourceChangedBatch",
		"ResourceDeleted",
		// Legacy compatibility
		"LocationAdded",
		"LocationRemoved",
		"FilesIndexed",
		"ThumbnailsGenerated",
		"FileOperationCompleted",
		"FilesModified",
	]
}

/// Daemon state - tracks if we started it or connected to existing one
struct DaemonState {
	started_by_us: bool,
	socket_path: PathBuf,
	data_dir: PathBuf,
	server_url: Option<String>,
	#[allow(dead_code)]
	server_shutdown: Option<tokio::sync::mpsc::Sender<()>>,
	daemon_process: Option<std::sync::Arc<tokio::sync::Mutex<Option<std::process::Child>>>>,
}

/// App state - stores global application state shared across all windows
struct AppState {
	current_library_id: Arc<RwLock<Option<String>>>,
	selected_file_ids: Arc<RwLock<Vec<String>>>,
}

/// Daemon status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonStatusResponse {
	is_running: bool,
	socket_path: String,
	server_url: Option<String>,
	started_by_us: bool,
}

/// Menu item state from frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MenuItemState {
	id: String,
	enabled: bool,
}

/// Menu state - stores references to menu items for dynamic updates
struct MenuState {
	items: Arc<RwLock<HashMap<String, MenuItem<tauri::Wry>>>>,
}

/// Called from frontend when app is ready to be shown
#[tauri::command]
async fn app_ready(app: AppHandle) {
	if let Some(window) = app.get_webview_window("main") {
		window.show().ok();
		window.set_focus().ok();
	}
}

/// Get the daemon socket path for the frontend to connect
#[tauri::command]
async fn get_daemon_socket(
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<String, String> {
	let state = state.read().await;
	Ok(state.socket_path.to_string_lossy().to_string())
}

/// Get the HTTP server URL for serving files/sidecars
#[tauri::command]
async fn get_server_url(
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<String, String> {
	let state = state.read().await;
	state
		.server_url
		.clone()
		.ok_or_else(|| "Server not started".to_string())
}

/// Set the current library ID in the window (legacy - injects into main window only)
#[tauri::command]
async fn set_library_id(
	app: AppHandle,
	library_id: String,
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<(), String> {
	if let Some(window) = app.get_webview_window("main") {
		let server_url = {
			let state = state.read().await;
			state.server_url.clone()
		};

		if let Some(server_url) = server_url {
			let script = format!(
				r#"window.__SPACEDRIVE_SERVER_URL__ = "{}"; window.__SPACEDRIVE_LIBRARY_ID__ = "{}";"#,
				server_url, library_id
			);

			window.eval(&script).map_err(|e| e.to_string())?;
			tracing::debug!("Injected library ID and server URL into window");
		}
	}
	Ok(())
}

/// Get the current library ID from app state (accessible by all windows)
#[tauri::command]
async fn get_current_library_id(
	app_state: tauri::State<'_, AppState>,
) -> Result<String, String> {
	let library_id = app_state.current_library_id.read().await;
	library_id
		.clone()
		.ok_or_else(|| "No library selected".to_string())
}

/// Set the current library ID in app state and emit to all windows
#[tauri::command]
async fn set_current_library_id(
	app: AppHandle,
	library_id: String,
	app_state: tauri::State<'_, AppState>,
	daemon_state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<(), String> {
	// Update app state
	*app_state.current_library_id.write().await = Some(library_id.clone());

	tracing::debug!("Library ID updated to: {}", library_id);

	// Persist library ID to disk for next app launch
	let data_dir = {
		let state = daemon_state.read().await;
		state.data_dir.clone()
	};
	let library_id_file = data_dir.join("current_library_id.txt");
	if let Err(e) = tokio::fs::write(&library_id_file, &library_id).await {
		tracing::warn!("Failed to persist library ID to disk: {}", e);
	} else {
		tracing::debug!("Persisted library ID to: {:?}", library_id_file);
	}

	// Also inject into all current windows for backwards compatibility
	let server_url = {
		let state = daemon_state.read().await;
		state.server_url.clone()
	};

	if let Some(server_url) = server_url {
		let script = format!(
			r#"window.__SPACEDRIVE_SERVER_URL__ = "{}"; window.__SPACEDRIVE_LIBRARY_ID__ = "{}";"#,
			server_url, library_id
		);

		// Inject into all windows
		for window in app.webview_windows().values() {
			if let Err(e) = window.eval(&script) {
				tracing::warn!("Failed to inject globals into window {}: {}", window.label(), e);
			}
		}
	}

	// Emit library-changed event to all windows
	app.emit("library-changed", &library_id)
		.map_err(|e| format!("Failed to emit library-changed event: {}", e))?;

	tracing::debug!("Emitted library-changed event to all windows");

	Ok(())
}

/// Get selected file IDs from app state
#[tauri::command]
async fn get_selected_file_ids(
	app_state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
	let file_ids = app_state.selected_file_ids.read().await;
	Ok(file_ids.clone())
}

/// Set selected file IDs in app state and emit to all windows
#[tauri::command]
async fn set_selected_file_ids(
	app: AppHandle,
	file_ids: Vec<String>,
	app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
	// Update app state
	*app_state.selected_file_ids.write().await = file_ids.clone();

	tracing::debug!("Selected file IDs updated: {} files", file_ids.len());

	// Emit selected-files-changed event to all windows
	app.emit("selected-files-changed", &file_ids)
		.map_err(|e| format!("Failed to emit selected-files-changed event: {}", e))?;

	Ok(())
}

/// Proxy daemon requests from frontend
#[tauri::command]
async fn daemon_request(
	request: serde_json::Value,
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<serde_json::Value, String> {
	let daemon_state = state.read().await;

	tracing::debug!("Proxying daemon request: {:?}", request);

	// Connect to daemon via Unix socket
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::UnixStream;

	let mut stream = UnixStream::connect(&daemon_state.socket_path)
		.await
		.map_err(|e| format!("Failed to connect to daemon: {}", e))?;

	// Send request
	let request_line = serde_json::to_string(&request)
		.map_err(|e| format!("Failed to serialize request: {}", e))?;

	tracing::debug!("Sending to daemon: {}", request_line);

	stream
		.write_all(format!("{}\n", request_line).as_bytes())
		.await
		.map_err(|e| format!("Failed to write request: {}", e))?;

	// Read response
	let mut reader = BufReader::new(stream);
	let mut response_line = String::new();

	reader
		.read_line(&mut response_line)
		.await
		.map_err(|e| format!("Failed to read response: {}", e))?;

	tracing::debug!("Received from daemon: {}", response_line.trim());

	serde_json::from_str(&response_line).map_err(|e| {
		format!(
			"Failed to parse response: {}. Raw: {}",
			e,
			response_line.trim()
		)
	})
}

/// Subscribe to daemon events and forward them to the frontend
#[tauri::command]
async fn subscribe_to_events(
	app: tauri::AppHandle,
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
	event_types: Option<Vec<String>>,
) -> Result<(), String> {
	let daemon_state = state.read().await;

	tracing::info!("Starting event subscription...");

	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::UnixStream;

	let socket_path = daemon_state.socket_path.clone();

	// Spawn background task to listen for events
	tauri::async_runtime::spawn(async move {
		let stream = match UnixStream::connect(&socket_path).await {
			Ok(s) => s,
			Err(e) => {
				tracing::error!("Failed to connect for events: {}", e);
				return;
			}
		};

		let (reader, mut writer) = stream.into_split();

		// Send subscription request
		// Frontend controls which events to subscribe to via event_types parameter
		// Falls back to default list if not provided (for backwards compatibility)
		let events = event_types.unwrap_or_else(|| {
			get_default_event_subscription().iter().map(|s| s.to_string()).collect()
		});

		let subscribe_request = json!({
			"Subscribe": {
				"event_types": events,
				"filter": null
			}
		});

		let request_line = format!("{}\n", serde_json::to_string(&subscribe_request).unwrap());
		if let Err(e) = writer.write_all(request_line.as_bytes()).await {
			tracing::error!("Failed to send subscription: {}", e);
			return;
		}

		tracing::info!("Event subscription active");

		// Listen for events and emit to frontend
		let mut reader = BufReader::new(reader);
		let mut buffer = String::new();

		loop {
			buffer.clear();
			match reader.read_line(&mut buffer).await {
				Ok(0) => {
					tracing::warn!("Event stream closed");
					break;
				}
				Ok(_) => {
					let line = buffer.trim();
					if line.is_empty() {
						continue;
					}

					match serde_json::from_str::<serde_json::Value>(line) {
						Ok(response) => {
							if let Some(event) = response.get("Event") {
								// tracing::info!("Emitting event to frontend: {:?}", event);
								// Emit to frontend via Tauri events
								if let Err(e) = app.emit("core-event", event) {
									tracing::error!("Failed to emit event: {}", e);
								}
							}
						}
						Err(e) => {
							tracing::error!("Failed to parse event: {}. Raw: {}", e, line);
						}
					}
				}
				Err(e) => {
					tracing::error!("Failed to read event: {}", e);
					break;
				}
			}
		}

		tracing::info!("Event subscription ended");
	});

	Ok(())
}

/// Update menu item states
#[tauri::command]
async fn update_menu_items(
	app: AppHandle,
	items: Vec<MenuItemState>,
) -> Result<(), String> {
	if let Some(menu_state) = app.try_state::<MenuState>() {
		let menu_items = menu_state.items.read().await;

		for item_state in items {
			if let Some(menu_item) = menu_items.get(&item_state.id) {
				menu_item.set_enabled(item_state.enabled).map_err(|e| {
					format!("Failed to set menu item '{}' enabled state: {}", item_state.id, e)
				})?;
			}
		}

		Ok(())
	} else {
		Err("Menu state not initialized".to_string())
	}
}

/// Get daemon status
#[tauri::command]
async fn get_daemon_status(
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<DaemonStatusResponse, String> {
	let daemon_state = state.read().await;
	let is_running = is_daemon_running(&daemon_state.socket_path).await;

	Ok(DaemonStatusResponse {
		is_running,
		socket_path: daemon_state.socket_path.to_string_lossy().to_string(),
		server_url: daemon_state.server_url.clone(),
		started_by_us: daemon_state.started_by_us,
	})
}

/// Start daemon as a background process
#[tauri::command]
async fn start_daemon_process(
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<(), String> {
	let (data_dir, socket_path) = {
		let daemon_state = state.read().await;
		(daemon_state.data_dir.clone(), daemon_state.socket_path.clone())
	};

	// Check if already running
	if is_daemon_running(&socket_path).await {
		return Err("Daemon is already running".to_string());
	}

	// Start the daemon
	let child = start_daemon(&data_dir, &socket_path).await?;

	// Store the process handle
	let mut daemon_state = state.write().await;
	daemon_state.started_by_us = true;
	daemon_state.daemon_process = Some(std::sync::Arc::new(tokio::sync::Mutex::new(Some(child))));

	Ok(())
}

/// Stop daemon process (only if we started it)
#[tauri::command]
async fn stop_daemon_process(
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<(), String> {
	let mut daemon_state = state.write().await;

	if !daemon_state.started_by_us {
		return Err("Cannot stop daemon we didn't start".to_string());
	}

	if let Some(process_arc) = daemon_state.daemon_process.take() {
		let mut process_lock = process_arc.lock().await;
		if let Some(mut child) = process_lock.take() {
			child.kill().map_err(|e| format!("Failed to kill daemon: {}", e))?;
			tracing::info!("Daemon process killed");
		}
	}

	daemon_state.started_by_us = false;
	Ok(())
}

/// Open macOS system settings for background items
#[tauri::command]
async fn open_macos_settings() -> Result<(), String> {
	#[cfg(target_os = "macos")]
	{
		std::process::Command::new("open")
			.arg("x-apple.systempreferences:com.apple.LoginItems-Settings.extension")
			.spawn()
			.map_err(|e| format!("Failed to open settings: {}", e))?;
	}

	#[cfg(not(target_os = "macos"))]
	{
		return Err("Not supported on this platform".to_string());
	}

	Ok(())
}

/// Check if daemon is running by trying to connect to it
async fn is_daemon_running(socket_path: &PathBuf) -> bool {
	use tokio::net::UnixStream;

	if !socket_path.exists() {
		return false;
	}

	// Try to actually connect to the socket
	match UnixStream::connect(socket_path).await {
		Ok(_) => {
			tracing::debug!("Successfully connected to daemon socket");
			true
		}
		Err(e) => {
			tracing::warn!(
				"Socket file exists but connection failed: {}. Will clean up stale socket.",
				e
			);
			// Remove stale socket file
			std::fs::remove_file(socket_path).ok();
			false
		}
	}
}

/// Start the daemon as a background process
async fn start_daemon(data_dir: &PathBuf, socket_path: &PathBuf) -> Result<std::process::Child, String> {
	// Find the daemon binary
	let daemon_path = if cfg!(debug_assertions) {
		// In dev mode, look in workspace target directory
		// Current exe is at: workspace/target/debug/spacedrive-tauri
		// Daemon is at: workspace/target/debug/sd-daemon
		let exe_path =
			std::env::current_exe().map_err(|e| format!("Failed to get current exe: {}", e))?;

		tracing::debug!("Current exe: {:?}", exe_path);

		let target_dir = exe_path.parent().ok_or("No parent directory for exe")?;

		let daemon_path = target_dir.join("sd-daemon");
		tracing::debug!("Looking for daemon at: {:?}", daemon_path);

		daemon_path
	} else {
		// In production, daemon should be in same directory as the app
		std::env::current_exe()
			.map_err(|e| format!("Failed to get current exe: {}", e))?
			.parent()
			.ok_or("No parent directory")?
			.join("sd-daemon")
	};

	tracing::info!("Starting daemon from: {:?}", daemon_path);

	if !daemon_path.exists() {
		return Err(format!(
			"Daemon binary not found at {:?}. Run 'cargo build --bin sd-daemon' first.",
			daemon_path
		));
	}

	let child = std::process::Command::new(daemon_path)
		.arg("--data-dir")
		.arg(data_dir)
		.stdout(std::process::Stdio::null())
		.stderr(std::process::Stdio::null())
		.spawn()
		.map_err(|e| format!("Failed to start daemon: {}", e))?;

	// Wait for socket to be created (daemon startup)
	for i in 0..30 {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		if socket_path.exists() {
			tracing::info!("Daemon socket created: {:?}", socket_path);
			return Ok(child);
		}
		if i == 10 {
			tracing::warn!("Daemon taking longer than expected to start...");
		}
	}

	Err("Daemon failed to start (socket not created after 3 seconds)".to_string())
}

fn setup_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
	use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder, PredefinedMenuItem};

	// Store menu items for dynamic updates
	let mut menu_items_map = HashMap::new();

	// Application menu (macOS standard)
	let app_menu = SubmenuBuilder::new(app, "Spacedrive")
		.item(&PredefinedMenuItem::about(app, None, None)?)
		.separator()
		.item(&PredefinedMenuItem::hide(app, None)?)
		.item(&PredefinedMenuItem::hide_others(app, None)?)
		.item(&PredefinedMenuItem::show_all(app, None)?)
		.separator()
		.item(&PredefinedMenuItem::quit(app, None)?)
		.build()?;

	// File menu with explorer actions
	let copy_item = MenuItemBuilder::with_id("copy", "Copy")
		.accelerator("Cmd+C")
		.enabled(false)
		.build(app)?;
	menu_items_map.insert("copy".to_string(), copy_item.clone());

	let paste_item = MenuItemBuilder::with_id("paste", "Paste")
		.accelerator("Cmd+V")
		.enabled(false)
		.build(app)?;
	menu_items_map.insert("paste".to_string(), paste_item.clone());

	let cut_item = MenuItemBuilder::with_id("cut", "Cut")
		.accelerator("Cmd+X")
		.enabled(false)
		.build(app)?;
	menu_items_map.insert("cut".to_string(), cut_item.clone());

	let duplicate_item = MenuItemBuilder::with_id("duplicate", "Duplicate")
		.accelerator("Cmd+D")
		.enabled(false)
		.build(app)?;
	menu_items_map.insert("duplicate".to_string(), duplicate_item.clone());

	let rename_item = MenuItemBuilder::with_id("rename", "Rename")
		.accelerator("Enter")
		.enabled(false)
		.build(app)?;
	menu_items_map.insert("rename".to_string(), rename_item.clone());

	let delete_item = MenuItemBuilder::with_id("delete", "Move to Trash")
		.accelerator("Cmd+Backspace")
		.enabled(false)
		.build(app)?;
	menu_items_map.insert("delete".to_string(), delete_item.clone());

	let file_menu = SubmenuBuilder::new(app, "File")
		.item(&copy_item)
		.item(&paste_item)
		.item(&cut_item)
		.item(&duplicate_item)
		.separator()
		.item(&rename_item)
		.separator()
		.item(&delete_item)
		.build()?;

	let view_menu = SubmenuBuilder::new(app, "View")
		.item(
			&MenuItemBuilder::with_id("drag-demo", "Drag Demo")
				.accelerator("Cmd+Shift+D")
				.build(app)?,
		)
		.item(
			&MenuItemBuilder::with_id("spacedrop", "Spacedrop")
				.accelerator("Cmd+Shift+S")
				.build(app)?,
		)
		.build()?;

	let menu = MenuBuilder::new(app)
		.item(&app_menu)
		.item(&file_menu)
		.item(&view_menu)
		.build()?;

	app.set_menu(menu)?;

	// Store menu items in app state
	let menu_state = MenuState {
		items: Arc::new(RwLock::new(menu_items_map)),
	};
	app.manage(menu_state);

	// Handle menu events
	let app_handle = app.clone();
	app.on_menu_event(move |_app, event| {
		let event_id = event.id().as_ref();
		match event_id {
			"drag-demo" => {
				let app_clone = app_handle.clone();
				tauri::async_runtime::spawn(async move {
					if let Err(e) =
						windows::show_window(app_clone, windows::SpacedriveWindow::DragDemo).await
					{
						tracing::error!("Failed to show drag demo: {}", e);
					}
				});
			}
			"spacedrop" => {
				let app_clone = app_handle.clone();
				tauri::async_runtime::spawn(async move {
					if let Err(e) =
						windows::show_window(app_clone, windows::SpacedriveWindow::Spacedrop).await
					{
						tracing::error!("Failed to show spacedrop: {}", e);
					}
				});
			}
			// File menu actions - emit events to frontend
			"copy" | "paste" | "cut" | "duplicate" | "rename" | "delete" => {
				if let Err(e) = app_handle.emit("menu-action", event_id) {
					tracing::error!("Failed to emit menu action: {}", e);
				}
			}
			_ => {}
		}
	});

	Ok(())
}

fn main() {
	// Initialize logging
	tracing_subscriber::registry()
		.with(
			tracing_subscriber::EnvFilter::try_from_default_env()
				.unwrap_or_else(|_| "info,sd_core=debug".into()),
		)
		.with(tracing_subscriber::fmt::layer())
		.init();

	tauri::Builder::default()
		.plugin(tauri_plugin_clipboard_manager::init())
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_fs::init())
		.plugin(tauri_plugin_os::init())
		.plugin(tauri_plugin_shell::init())
		.invoke_handler(tauri::generate_handler![
			app_ready,
			get_daemon_socket,
			get_server_url,
			set_library_id,
			get_current_library_id,
			set_current_library_id,
			get_selected_file_ids,
			set_selected_file_ids,
			daemon_request,
			subscribe_to_events,
			update_menu_items,
			get_daemon_status,
			start_daemon_process,
			stop_daemon_process,
			open_macos_settings,
			windows::show_window,
			windows::close_window,
			windows::list_windows,
			windows::apply_macos_styling,
			windows::position_context_menu,
			drag::begin_drag,
			drag::end_drag,
			drag::get_drag_session,
			drag::force_clear_drag_state,
			files::reveal_file,
			files::get_sidecar_path
		])
		.setup(|app| {
			// Setup native menu
			if let Err(e) = setup_menu(&app.handle()) {
				tracing::warn!("Failed to setup menu: {}", e);
			}
			tracing::info!("Spacedrive Tauri app starting...");

			// Apply macOS-specific window customizations
			#[cfg(target_os = "macos")]
			{
				if let Some(window) = app.get_webview_window("main") {
					tracing::info!("Applying macOS window customizations...");
					match window.ns_window() {
						Ok(ns_window) => unsafe {
							tracing::debug!("Setting titlebar style...");
							sd_desktop_macos::set_titlebar_style(&ns_window, false);
							tracing::debug!("Locking app theme...");
							sd_desktop_macos::lock_app_theme(1); // 1 = Dark theme
							tracing::info!("macOS customizations applied successfully");
						},
						Err(e) => {
							tracing::warn!("Could not get NSWindow handle: {}", e);
						}
					}
				}

				// Setup drag ended callback
				let app_handle = app.handle().clone();
				sd_desktop_macos::set_drag_ended_callback(move |session_id: &str, was_dropped: bool| {
					tracing::info!("[DRAG] Swift callback: session_id={}, was_dropped={}", session_id, was_dropped);
					let coordinator = app_handle.state::<drag::DragCoordinator>();
					let result = if was_dropped {
						drag::DragResult::Dropped {
							operation: drag::DragOperation::Copy,
							target: None,
						}
					} else {
						drag::DragResult::Cancelled
					};
					coordinator.end_drag(&app_handle, result);

					// Hide and then close the overlay window after a delay to avoid focus issues
					let overlay_label = format!("drag-overlay-{}", session_id);
					if let Some(overlay) = app_handle.get_webview_window(&overlay_label) {
						tracing::debug!("[DRAG] Hiding overlay window from callback: {}", overlay_label);
						// First hide it immediately
						overlay.hide().ok();

						// Then close it after a short delay to avoid window focus flashing
						let overlay_clone = overlay.clone();
						std::thread::spawn(move || {
							std::thread::sleep(std::time::Duration::from_millis(100));
							overlay_clone.close().ok();
						});
					} else {
						tracing::warn!("[DRAG] Overlay window not found in callback: {}", overlay_label);
					}
				});
				tracing::info!("Drag ended callback registered");
			}

			// Get data directory (use default Spacedrive location)
			let data_dir =
				sd_core::config::default_data_dir().expect("Failed to get default data directory");

			let socket_path = data_dir.join("daemon/daemon.sock");

			// Initialize state immediately (before async operations)
			let daemon_state = Arc::new(RwLock::new(DaemonState {
				started_by_us: false,
				socket_path: socket_path.clone(),
				data_dir: data_dir.clone(),
				server_url: None,
				server_shutdown: None,
				daemon_process: None,
			}));

			// Initialize app state for library ID (shared across all windows)
			// Try to load persisted library ID from disk
			let persisted_library_id = {
				let library_id_file = data_dir.join("current_library_id.txt");
				if library_id_file.exists() {
					match std::fs::read_to_string(&library_id_file) {
						Ok(id) => {
							tracing::info!("Loaded persisted library ID: {}", id);
							Some(id.trim().to_string())
						}
						Err(e) => {
							tracing::warn!("Failed to read persisted library ID: {}", e);
							None
						}
					}
				} else {
					None
				}
			};

			let app_state = AppState {
				current_library_id: Arc::new(RwLock::new(persisted_library_id)),
				selected_file_ids: Arc::new(RwLock::new(Vec::new())),
			};

			app.manage(daemon_state.clone());
			app.manage(app_state);
			app.manage(drag::DragCoordinator::new());

			let _handle = app.handle().clone();

			// Initialize daemon connection in background
			tauri::async_runtime::spawn(async move {
				tracing::info!("Data directory: {:?}", data_dir);
				tracing::info!("Socket path: {:?}", socket_path);

				// Start HTTP server for serving files/sidecars
				match server::start_server(data_dir.clone()).await {
					Ok((server_url, shutdown_tx)) => {
						tracing::info!("HTTP server started at {}", server_url);
						let mut state = daemon_state.write().await;
						state.server_url = Some(server_url);
						state.server_shutdown = Some(shutdown_tx);
					}
					Err(e) => {
						tracing::error!("Failed to start HTTP server: {}", e);
					}
				}

				// Ensure daemon directory exists
				if let Some(parent) = socket_path.parent() {
					std::fs::create_dir_all(parent).ok();
				}

				let (started_by_us, child_process) = if is_daemon_running(&socket_path).await {
					tracing::info!("Daemon already running, connecting to existing instance");
					(false, None)
				} else {
					tracing::info!("No daemon running, starting new instance");
					match start_daemon(&data_dir, &socket_path).await {
						Ok(child) => {
							(true, Some(std::sync::Arc::new(tokio::sync::Mutex::new(Some(child)))))
						}
						Err(e) => {
							tracing::error!("Failed to start daemon: {}", e);
							return;
						}
					}
				};

				// Update daemon state
				let mut state = daemon_state.write().await;
				state.started_by_us = started_by_us;
				state.daemon_process = child_process;

				tracing::info!("Daemon connection established");
			});

			// In dev mode, show window immediately
			#[cfg(debug_assertions)]
			{
				if let Some(window) = app.get_webview_window("main") {
					window.show().ok();
					window.set_focus().ok();
				}
			}

			Ok(())
		})
		.on_window_event(|window, event| {
			// Update titlebar on fullscreen change (macOS)
			#[cfg(target_os = "macos")]
			if let tauri::WindowEvent::Resized(_) = event {
				if let Ok(is_fullscreen) = window.is_fullscreen() {
					if let Ok(ns_window) = window.ns_window() {
						unsafe {
							sd_desktop_macos::set_titlebar_style(&ns_window, is_fullscreen);
						}
					}
				}
			}

			if let tauri::WindowEvent::CloseRequested { .. } = event {
				// Get daemon state
				let app = window.app_handle().clone();
				if let Some(state) = app.try_state::<Arc<RwLock<DaemonState>>>() {
					let state = state.inner().clone();
					tauri::async_runtime::spawn(async move {
						let daemon_state = state.read().await;

						// Only stop daemon if we started it
						if daemon_state.started_by_us {
							tracing::info!("App closing, shutting down daemon we started");
							// Daemon will be stopped when process exits
							// Could implement graceful shutdown here if needed
						} else {
							tracing::info!("App closing, leaving existing daemon running");
						}
					});
				}
			}
		})
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
