// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod drag;
mod file_opening;
mod files;
mod keybinds;
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
use tokio::sync::oneshot;
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
	socket_addr: String,
	data_dir: PathBuf,
	server_url: Option<String>,
	#[allow(dead_code)]
	server_shutdown: Option<tokio::sync::mpsc::Sender<()>>,
	daemon_process: Option<std::sync::Arc<tokio::sync::Mutex<Option<std::process::Child>>>>,
}

/// Daemon connection pool - maintains ONE persistent connection for all subscriptions
/// Multiplexes Subscribe/Unsubscribe messages over a single TCP connection
#[allow(dead_code)]
struct DaemonConnectionPool {
	socket_addr: String,
	writer: Arc<tokio::sync::Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>>,
	subscriptions: Arc<RwLock<HashMap<u64, ()>>>,
	counter: std::sync::atomic::AtomicU64,
	initialized: Arc<tokio::sync::Mutex<bool>>,
}

#[allow(dead_code)]
impl DaemonConnectionPool {
	fn new(socket_addr: String) -> Self {
		Self {
			socket_addr,
			writer: Arc::new(tokio::sync::Mutex::new(None)),
			subscriptions: Arc::new(RwLock::new(HashMap::new())),
			counter: std::sync::atomic::AtomicU64::new(0),
			initialized: Arc::new(tokio::sync::Mutex::new(false)),
		}
	}

	async fn reset(&self) {
		let mut initialized = self.initialized.lock().await;
		*initialized = false;
		*self.writer.lock().await = None;
		self.subscriptions.write().await.clear();
		tracing::info!("Connection pool reset");
	}

	async fn ensure_connected(&self, app: &AppHandle) -> Result<(), String> {
		let mut initialized = self.initialized.lock().await;

		if *initialized {
			return Ok(());
		}

		tracing::info!("Initializing persistent daemon connection");

		use tokio::io::{AsyncBufReadExt, BufReader};
		use tokio::net::TcpStream;

		let stream = TcpStream::connect(&self.socket_addr)
			.await
			.map_err(|e| format!("Failed to connect to daemon: {}", e))?;

		let (reader, writer) = stream.into_split();

		*self.writer.lock().await = Some(writer);

		// Emit connection event
		let _ = app.emit("daemon-connected", ());

		// Spawn persistent reader task that broadcasts to all listeners
		let app_clone = app.clone();
		tokio::spawn(async move {
			let mut reader = BufReader::new(reader);
			let mut buffer = String::new();

			loop {
				buffer.clear();
				match reader.read_line(&mut buffer).await {
					Ok(0) => {
						tracing::warn!("Daemon connection closed");
						let _ = app_clone.emit("daemon-disconnected", ());
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
									// Broadcast to all frontend listeners
									let _ = app_clone.emit("core-event", event);
								}
							}
							Err(e) => {
								tracing::error!("Failed to parse event: {}", e);
							}
						}
					}
					Err(e) => {
						tracing::error!("Failed to read from daemon: {}", e);
						let _ = app_clone.emit("daemon-disconnected", ());
						break;
					}
				}
			}

			tracing::warn!("Daemon connection reader ended");
		});

		*initialized = true;
		tracing::info!("Persistent daemon connection ready");
		Ok(())
	}

	fn next_id(&self) -> u64 {
		self.counter
			.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
	}

	async fn subscribe(
		&self,
		subscription_id: u64,
		event_types: Vec<String>,
		filter: Option<serde_json::Value>,
	) -> Result<(), String> {
		use tokio::io::AsyncWriteExt;

		let mut writer_guard = self.writer.lock().await;
		let writer = writer_guard.as_mut().ok_or("Connection not initialized")?;

		let subscribe_request = json!({
			"Subscribe": {
				"event_types": event_types,
				"filter": filter
			}
		});

		let request_line = format!("{}\n", serde_json::to_string(&subscribe_request).unwrap());
		writer
			.write_all(request_line.as_bytes())
			.await
			.map_err(|e| format!("Failed to send Subscribe: {}", e))?;

		self.subscriptions.write().await.insert(subscription_id, ());

		let total = self.subscriptions.read().await.len();
		tracing::info!(
			subscription_id = subscription_id,
			total_subscriptions = total,
			"Subscribe sent over persistent connection"
		);

		Ok(())
	}

	async fn unsubscribe(&self, subscription_id: u64) -> Result<(), String> {
		use tokio::io::AsyncWriteExt;

		if self
			.subscriptions
			.write()
			.await
			.remove(&subscription_id)
			.is_none()
		{
			return Err(format!("Subscription {} not found", subscription_id));
		}

		let mut writer_guard = self.writer.lock().await;
		let writer = writer_guard.as_mut().ok_or("Connection not initialized")?;

		let unsubscribe_request = json!("Unsubscribe");
		let request_line = format!("{}\n", serde_json::to_string(&unsubscribe_request).unwrap());

		writer
			.write_all(request_line.as_bytes())
			.await
			.map_err(|e| format!("Failed to send Unsubscribe: {}", e))?;

		let remaining = self.subscriptions.read().await.len();
		tracing::info!(
			subscription_id = subscription_id,
			remaining_subscriptions = remaining,
			"Unsubscribe sent over persistent connection"
		);

		Ok(())
	}
}

/// Manages active subscriptions and their cancellation channels
struct SubscriptionManager {
	subscriptions: Arc<RwLock<HashMap<u64, oneshot::Sender<()>>>>,
	counter: std::sync::atomic::AtomicU64,
}

impl SubscriptionManager {
	fn new() -> Self {
		Self {
			subscriptions: Arc::new(RwLock::new(HashMap::new())),
			counter: std::sync::atomic::AtomicU64::new(0),
		}
	}

	fn next_id(&self) -> u64 {
		self.counter
			.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
	}

	async fn register(&self, subscription_id: u64, cancel_tx: oneshot::Sender<()>) {
		self.subscriptions
			.write()
			.await
			.insert(subscription_id, cancel_tx);
	}

	async fn cancel(&self, subscription_id: u64) -> bool {
		if let Some(cancel_tx) = self.subscriptions.write().await.remove(&subscription_id) {
			// Send cancellation signal (ignore if receiver is already dropped)
			let _ = cancel_tx.send(());
			true
		} else {
			false
		}
	}

	async fn cancel_all(&self) {
		let mut subscriptions = self.subscriptions.write().await;
		let count = subscriptions.len();
		for (_, cancel_tx) in subscriptions.drain() {
			let _ = cancel_tx.send(());
		}
		tracing::info!("Cancelled {} subscriptions", count);
	}

	async fn get_active_count(&self) -> usize {
		self.subscriptions.read().await.len()
	}
}

/// App state - stores global application state shared across all windows
struct AppState {
	current_library_id: Arc<RwLock<Option<String>>>,
	selected_file_ids: Arc<RwLock<Vec<String>>>,
	connection_pool: Arc<DaemonConnectionPool>,
	subscription_manager: SubscriptionManager,
}

/// Daemon status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonStatusResponse {
	is_running: bool,
	socket_addr: String,
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
async fn app_ready(window: tauri::Window) {
	window.show().ok();
	window.set_focus().ok();
}

/// Get the daemon socket address for the frontend to connect
#[tauri::command]
async fn get_daemon_socket(
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<String, String> {
	let state = state.read().await;
	Ok(state.socket_addr.clone())
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
async fn get_current_library_id(app_state: tauri::State<'_, AppState>) -> Result<String, String> {
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
				tracing::warn!(
					"Failed to inject globals into window {}: {}",
					window.label(),
					e
				);
			}
		}
	}

	// Emit library-changed event to all windows
	app.emit("library-changed", &library_id)
		.map_err(|e| format!("Failed to emit library-changed event: {}", e))?;

	tracing::debug!("Emitted library-changed event to all windows");

	Ok(())
}

/// Validate that the current library exists, reset state if it doesn't
async fn validate_and_reset_library_if_needed(
	app: AppHandle,
	current_library_id_arc: &Arc<RwLock<Option<String>>>,
	daemon_state: &Arc<RwLock<DaemonState>>,
	data_dir: &PathBuf,
) -> Result<(), String> {
	let current_library_id = {
		let library_id = current_library_id_arc.read().await;
		library_id.clone()
	};

	let Some(library_id) = current_library_id else {
		// No library selected, nothing to validate
		return Ok(());
	};

	// Query daemon for list of libraries
	let request = json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "query:libraries.list",
		"params": {
			"input": {
				"include_stats": false
			}
		}
	});

	let socket_addr = {
		let state = daemon_state.read().await;
		state.socket_addr.clone()
	};

	// Use direct TCP communication (same as daemon_request but without Tauri State)
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::TcpStream;

	let mut stream = TcpStream::connect(&socket_addr)
		.await
		.map_err(|e| format!("Failed to connect to daemon: {}", e))?;

	let request_line = serde_json::to_string(&request)
		.map_err(|e| format!("Failed to serialize request: {}", e))?;

	stream
		.write_all(format!("{}\n", request_line).as_bytes())
		.await
		.map_err(|e| format!("Failed to write request: {}", e))?;

	let mut reader = BufReader::new(stream);
	let mut response_line = String::new();

	reader
		.read_line(&mut response_line)
		.await
		.map_err(|e| format!("Failed to read response: {}", e))?;

	let response: serde_json::Value = serde_json::from_str(&response_line).map_err(|e| {
		format!(
			"Failed to parse response: {}. Raw: {}",
			e,
			response_line.trim()
		)
	})?;

	// Parse response to get library list
	let libraries: Vec<serde_json::Value> = response
		.get("result")
		.and_then(|r| r.as_array())
		.ok_or_else(|| "Invalid response format from libraries.list query".to_string())?
		.clone();

	// Check if current library ID exists in the list
	let library_exists = libraries.iter().any(|lib| {
		lib.get("id")
			.and_then(|id| id.as_str())
			.map(|id| id == library_id)
			.unwrap_or(false)
	});

	if !library_exists {
		tracing::warn!(
			"Current library {} no longer exists, resetting library state",
			library_id
		);

		// Clear library ID from app state
		*current_library_id_arc.write().await = None;

		// Remove persisted library ID file
		let library_id_file = data_dir.join("current_library_id.txt");
		if let Err(e) = tokio::fs::remove_file(&library_id_file).await {
			tracing::warn!("Failed to remove persisted library ID file: {}", e);
		} else {
			tracing::debug!("Removed persisted library ID file: {:?}", library_id_file);
		}

		// Emit library-changed event with empty string to indicate no library (frontend uses Platform abstraction)
		if let Err(e) = app.emit("library-changed", "") {
			tracing::warn!("Failed to emit library-changed event: {}", e);
		}
	}

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

	// Connect to daemon via TCP
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::TcpStream;

	let stream = TcpStream::connect(&daemon_state.socket_addr)
		.await
		.map_err(|e| format!("Failed to connect to daemon: {}", e))?;

	let (reader, mut writer) = stream.into_split();

	// Send request
	let request_line = serde_json::to_string(&request)
		.map_err(|e| format!("Failed to serialize request: {}", e))?;

	tracing::debug!("Sending to daemon: {}", request_line);

	writer
		.write_all(format!("{}\n", request_line).as_bytes())
		.await
		.map_err(|e| format!("Failed to write request: {}", e))?;

	// Read response
	let mut buf_reader = BufReader::new(reader);
	let mut response_line = String::new();

	buf_reader
		.read_line(&mut response_line)
		.await
		.map_err(|e| format!("Failed to read response: {}", e))?;

	tracing::debug!("Received from daemon: {}", response_line.trim());

	// Explicitly close the connection by dropping both halves
	drop(writer);
	drop(buf_reader);

	serde_json::from_str(&response_line).map_err(|e| {
		format!(
			"Failed to parse response: {}. Raw: {}",
			e,
			response_line.trim()
		)
	})
}

/// Subscribe to daemon events and forward them to the frontend
/// Returns a subscription ID that can be used to unsubscribe
#[tauri::command]
#[allow(non_snake_case)]
async fn subscribe_to_events(
	app: tauri::AppHandle,
	daemon_state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
	app_state: tauri::State<'_, AppState>,
	eventTypes: Option<Vec<String>>,
	filter: Option<serde_json::Value>,
) -> Result<u64, String> {
	let daemon_state = daemon_state.read().await;

	// Generate unique subscription ID
	let subscription_id = app_state.subscription_manager.next_id();

	tracing::info!(
		subscription_id = subscription_id,
		"Starting event subscription with filter: {:?}, eventTypes: {:?}",
		filter,
		eventTypes
	);

	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::TcpStream;
	use tokio::sync::oneshot;

	let socket_addr = daemon_state.socket_addr.clone();

	// Create cancellation channel
	let (cancel_tx, mut cancel_rx) = oneshot::channel::<()>();

	// Register the cancellation sender
	app_state
		.subscription_manager
		.register(subscription_id, cancel_tx)
		.await;

	// Spawn background task to listen for events
	tauri::async_runtime::spawn(async move {
		tracing::debug!(
			subscription_id = subscription_id,
			"Creating TCP connection for subscription"
		);
		let mut stream = match TcpStream::connect(&socket_addr).await {
			Ok(s) => s,
			Err(e) => {
				tracing::error!("Failed to connect for events: {}", e);
				return;
			}
		};

		let (reader, mut writer) = stream.split();

		// Send subscription request
		// Frontend controls which events to subscribe to via eventTypes parameter
		// Falls back to default list if not provided (for backwards compatibility)
		let events = eventTypes.unwrap_or_else(|| {
			get_default_event_subscription()
				.iter()
				.map(|s| s.to_string())
				.collect()
		});

		let subscribe_request = json!({
			"Subscribe": {
				"event_types": events,
				"filter": filter
			}
		});

		let request_line = format!("{}\n", serde_json::to_string(&subscribe_request).unwrap());
		if let Err(e) = writer.write_all(request_line.as_bytes()).await {
			tracing::error!("Failed to send subscription: {}", e);
			return;
		}

		tracing::info!(
			subscription_id = subscription_id,
			"Event subscription active"
		);

		// Listen for events and emit to frontend
		let mut reader = BufReader::new(reader);
		let mut buffer = String::new();

		loop {
			buffer.clear();

			tokio::select! {
				// Check for cancellation
				_ = &mut cancel_rx => {
					tracing::info!(subscription_id = subscription_id, "Subscription cancelled by frontend");
					break;
				}

				// Read events from daemon
				result = reader.read_line(&mut buffer) => {
					match result {
						Ok(0) => {
							tracing::warn!(subscription_id = subscription_id, "Event stream closed");
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
										// Emit to frontend via Tauri events
										if let Err(e) = app.emit("core-event", event) {
											tracing::error!(subscription_id = subscription_id, "Failed to emit event: {}", e);
										}
									}
								}
								Err(e) => {
									tracing::error!(subscription_id = subscription_id, "Failed to parse event: {}. Raw: {}", e, line);
								}
							}
						}
						Err(e) => {
							tracing::error!(subscription_id = subscription_id, "Failed to read event: {}", e);
							break;
						}
					}
				}
			}
		}

		tracing::info!(
			subscription_id = subscription_id,
			"Event subscription ended, sending Unsubscribe"
		);

		// Send Unsubscribe request to daemon to clean up connection
		let unsubscribe_request = json!("Unsubscribe");
		let unsubscribe_line =
			format!("{}\n", serde_json::to_string(&unsubscribe_request).unwrap());
		if let Err(e) = writer.write_all(unsubscribe_line.as_bytes()).await {
			tracing::warn!(
				subscription_id = subscription_id,
				"Failed to send Unsubscribe: {}",
				e
			);
		} else {
			tracing::info!(
				subscription_id = subscription_id,
				"Unsubscribe sent successfully"
			);
		}

		// Explicitly shutdown and drop the stream to close the TCP connection
		drop(writer);
		drop(reader);
		tracing::info!(subscription_id = subscription_id, "TCP connection closed");
	});

	Ok(subscription_id)
}

/// Unsubscribe from daemon events
#[tauri::command]
async fn unsubscribe_from_events(
	app_state: tauri::State<'_, AppState>,
	subscription_id: u64,
) -> Result<(), String> {
	let cancelled = app_state.subscription_manager.cancel(subscription_id).await;
	if cancelled {
		tracing::info!(
			subscription_id = subscription_id,
			"Unsubscribed successfully"
		);
		Ok(())
	} else {
		Err(format!("Subscription {} not found", subscription_id))
	}
}

/// Cleanup all active subscriptions (useful for app reloads)
#[tauri::command]
async fn cleanup_all_connections(app_state: tauri::State<'_, AppState>) -> Result<(), String> {
	let count = app_state.subscription_manager.get_active_count().await;
	tracing::info!("Cleaning up {} active subscriptions", count);
	app_state.subscription_manager.cancel_all().await;
	Ok(())
}

/// Get active subscription count (for debugging)
#[tauri::command]
async fn get_active_subscriptions(app_state: tauri::State<'_, AppState>) -> Result<usize, String> {
	Ok(app_state.subscription_manager.get_active_count().await)
}

/// Update menu item states
#[tauri::command]
async fn update_menu_items(app: AppHandle, items: Vec<MenuItemState>) -> Result<(), String> {
	if let Some(menu_state) = app.try_state::<MenuState>() {
		let menu_items = menu_state.items.read().await;

		for item_state in items {
			if let Some(menu_item) = menu_items.get(&item_state.id) {
				menu_item.set_enabled(item_state.enabled).map_err(|e| {
					format!(
						"Failed to set menu item '{}' enabled state: {}",
						item_state.id, e
					)
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
	let is_running = is_daemon_running(&daemon_state.socket_addr).await;

	Ok(DaemonStatusResponse {
		is_running,
		socket_addr: daemon_state.socket_addr.clone(),
		server_url: daemon_state.server_url.clone(),
		started_by_us: daemon_state.started_by_us,
	})
}

/// Start daemon as a background process
#[tauri::command]
async fn start_daemon_process(
	app: tauri::AppHandle,
	state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
) -> Result<(), String> {
	let (data_dir, socket_addr) = {
		let daemon_state = state.read().await;
		(
			daemon_state.data_dir.clone(),
			daemon_state.socket_addr.clone(),
		)
	};

	// Check if already running
	if is_daemon_running(&socket_addr).await {
		return Err("Daemon is already running".to_string());
	}

	// Emit starting event
	let _ = app.emit("daemon-starting", ());

	// Start the daemon
	let child = start_daemon(&data_dir, &socket_addr).await?;

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
			child
				.kill()
				.map_err(|e| format!("Failed to kill daemon: {}", e))?;
			tracing::info!("Daemon process killed");
		}
	}

	daemon_state.started_by_us = false;
	Ok(())
}

/// Check if daemon is installed as a service (LaunchAgent on macOS, systemd on Linux)
#[tauri::command]
async fn check_daemon_installed() -> Result<bool, String> {
	#[cfg(target_os = "macos")]
	{
		let home =
			std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
		let plist_path =
			std::path::PathBuf::from(home).join("Library/LaunchAgents/com.spacedrive.daemon.plist");
		let exists = plist_path.exists();
		tracing::info!(
			"Checking daemon installation at {}: {}",
			plist_path.display(),
			exists
		);
		Ok(exists)
	}

	#[cfg(target_os = "linux")]
	{
		let home =
			std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
		let service_path =
			std::path::PathBuf::from(home).join(".config/systemd/user/spacedrive-daemon.service");
		Ok(service_path.exists())
	}

	#[cfg(target_os = "windows")]
	{
		// On Windows, check if scheduled task exists
		let output = std::process::Command::new("schtasks")
			.args(&["/Query", "/TN", "SpacedriveDaemon", "/FO", "LIST"])
			.output()
			.map_err(|e| format!("Failed to query scheduled task: {}", e))?;

		Ok(output.status.success())
	}

	#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
	{
		Ok(false)
	}
}

/// Install daemon as a service (LaunchAgent on macOS, systemd on Linux)
#[tauri::command]
async fn install_daemon_service(
	app: tauri::AppHandle,
	daemon_state: tauri::State<'_, Arc<RwLock<DaemonState>>>,
	app_state: tauri::State<'_, AppState>,
) -> Result<(), String> {
	let (data_dir, socket_addr) = {
		let state = daemon_state.read().await;
		(state.data_dir.clone(), state.socket_addr.clone())
	};

	tracing::info!("Installing daemon as service");

	// Stop any existing daemon child process first
	{
		let mut state = daemon_state.write().await;
		if let Some(process_arc) = state.daemon_process.take() {
			tracing::info!("Stopping existing daemon child process");
			let mut process_lock = process_arc.lock().await;
			if let Some(mut child) = process_lock.take() {
				let _ = child.kill();
			}
		}
	}

	// Emit starting event since installation starts the daemon
	let _ = app.emit("daemon-starting", ());
	tracing::info!("Emitted daemon-starting event");

	#[cfg(target_os = "macos")]
	{
		use std::io::Write;

		let home =
			std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
		let launch_agents_dir = std::path::PathBuf::from(&home).join("Library/LaunchAgents");

		std::fs::create_dir_all(&launch_agents_dir)
			.map_err(|e| format!("Failed to create LaunchAgents directory: {}", e))?;

		let plist_path = launch_agents_dir.join("com.spacedrive.daemon.plist");
		tracing::info!("Creating plist at: {}", plist_path.display());

		let daemon_path = std::env::current_exe()
			.map_err(|e| format!("Failed to get current exe: {}", e))?
			.parent()
			.ok_or_else(|| "Could not determine binary directory".to_string())?
			.join("sd-daemon");

		if !daemon_path.exists() {
			return Err(format!(
				"Daemon binary not found at {}",
				daemon_path.display()
			));
		}

		let log_dir = data_dir.join("logs");
		std::fs::create_dir_all(&log_dir)
			.map_err(|e| format!("Failed to create logs directory: {}", e))?;

		let plist_content = format!(
			r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>com.spacedrive.daemon</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>--data-dir</string>
		<string>{}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>StandardOutPath</key>
	<string>{}</string>
	<key>StandardErrorPath</key>
	<string>{}</string>
</dict>
</plist>"#,
			daemon_path.display(),
			data_dir.display(),
			log_dir.join("daemon.out.log").display(),
			log_dir.join("daemon.err.log").display()
		);

		let mut file = std::fs::File::create(&plist_path)
			.map_err(|e| format!("Failed to create plist file: {}", e))?;
		file.write_all(plist_content.as_bytes())
			.map_err(|e| format!("Failed to write plist file: {}", e))?;

		// Unload any existing service first
		tracing::info!("Unloading any existing service");
		let _ = std::process::Command::new("launchctl")
			.args(["unload", plist_path.to_str().unwrap()])
			.output();

		// Load the service (this starts the daemon)
		tracing::info!("Loading service with launchctl");
		let output = std::process::Command::new("launchctl")
			.args(["load", plist_path.to_str().unwrap()])
			.output()
			.map_err(|e| format!("Failed to load service: {}", e))?;

		tracing::info!(
			"launchctl load output: {:?}",
			String::from_utf8_lossy(&output.stdout)
		);
		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			tracing::error!("launchctl load failed: {:?}", stderr);
			return Err(format!("Failed to load daemon service: {}", stderr));
		}

		// Update daemon state - we no longer own the process
		let mut state = daemon_state.write().await;
		state.started_by_us = false;
		state.daemon_process = None;
		tracing::info!("Updated daemon state: started_by_us = false");
		drop(state);

		// Reset connection pool so it can reconnect to the service-managed daemon
		tracing::info!("Resetting connection pool to reconnect to service daemon");
		app_state.connection_pool.reset().await;

		// Wait for daemon to start and become available
		tracing::info!("Waiting for daemon to become available...");
		for i in 0..30 {
			tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
			if is_daemon_running(&socket_addr).await {
				tracing::info!("Daemon is running after {} attempts", i + 1);
				break;
			}
			if i == 29 {
				return Err("Daemon failed to start after installing service".to_string());
			}
		}

		// Trigger reconnection
		tracing::info!("Triggering reconnection");
		app_state.connection_pool.ensure_connected(&app).await?;

		Ok(())
	}

	#[cfg(target_os = "linux")]
	{
		use std::io::Write;

		let home =
			std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
		let systemd_dir = std::path::PathBuf::from(&home).join(".config/systemd/user");

		std::fs::create_dir_all(&systemd_dir)
			.map_err(|e| format!("Failed to create systemd directory: {}", e))?;

		let service_path = systemd_dir.join("spacedrive-daemon.service");

		let daemon_path = std::env::current_exe()
			.map_err(|e| format!("Failed to get current exe: {}", e))?
			.parent()
			.ok_or_else(|| "Could not determine binary directory".to_string())?
			.join("sd-daemon");

		if !daemon_path.exists() {
			return Err(format!(
				"Daemon binary not found at {}",
				daemon_path.display()
			));
		}

		let service_content = format!(
			r#"[Unit]
Description=Spacedrive Daemon
After=network.target

[Service]
Type=simple
ExecStart={} --data-dir {}
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=default.target
"#,
			daemon_path.display(),
			data_dir.display()
		);

		let mut file = std::fs::File::create(&service_path)
			.map_err(|e| format!("Failed to create service file: {}", e))?;
		file.write_all(service_content.as_bytes())
			.map_err(|e| format!("Failed to write service file: {}", e))?;

		// Enable and start the service
		let output = std::process::Command::new("systemctl")
			.args(&["--user", "daemon-reload"])
			.output()
			.map_err(|e| format!("Failed to reload systemd: {}", e))?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			return Err(format!("Failed to reload systemd: {}", stderr));
		}

		let output = std::process::Command::new("systemctl")
			.args(&["--user", "enable", "spacedrive-daemon.service"])
			.output()
			.map_err(|e| format!("Failed to enable service: {}", e))?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			return Err(format!("Failed to enable service: {}", stderr));
		}

		let output = std::process::Command::new("systemctl")
			.args(&["--user", "start", "spacedrive-daemon.service"])
			.output()
			.map_err(|e| format!("Failed to start service: {}", e))?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			return Err(format!("Failed to start daemon service: {}", stderr));
		}

		// Update daemon state - we no longer own the process
		let mut state = daemon_state.write().await;
		state.started_by_us = false;
		state.daemon_process = None;
		tracing::info!("Updated daemon state: started_by_us = false");
		drop(state);

		// Reset connection pool so it can reconnect to the service-managed daemon
		tracing::info!("Resetting connection pool to reconnect to service daemon");
		app_state.connection_pool.reset().await;

		// Wait for daemon to start and become available
		tracing::info!("Waiting for daemon to become available...");
		for i in 0..30 {
			tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
			if is_daemon_running(&socket_addr).await {
				tracing::info!("Daemon is running after {} attempts", i + 1);
				break;
			}
			if i == 29 {
				return Err("Daemon failed to start after installing service".to_string());
			}
		}

		// Trigger reconnection
		tracing::info!("Triggering reconnection");
		app_state.connection_pool.ensure_connected(&app).await?;

		Ok(())
	}

	#[cfg(target_os = "windows")]
	{
		use std::io::Write;

		tracing::info!("Installing daemon as Windows scheduled task");

		// Stop any existing daemon child process first
		{
			let mut state = daemon_state.write().await;
			if let Some(process_arc) = state.daemon_process.take() {
				tracing::info!("Stopping existing daemon child process");
				let mut process_lock = process_arc.lock().await;
				if let Some(mut child) = process_lock.take() {
					let _ = child.kill();
				}
			}
		}

		// Emit starting event since installation starts the daemon
		let _ = app.emit("daemon-starting", ());
		tracing::info!("Emitted daemon-starting event");

		let daemon_path = std::env::current_exe()
			.map_err(|e| format!("Failed to get current exe: {}", e))?
			.parent()
			.ok_or_else(|| "Could not determine binary directory".to_string())?
			.join("sd-daemon.exe");

		if !daemon_path.exists() {
			return Err(format!(
				"Daemon binary not found at {}",
				daemon_path.display()
			));
		}

		// Delete existing task if it exists
		let _ = std::process::Command::new("schtasks")
			.args(&["/Delete", "/TN", "SpacedriveDaemon", "/F"])
			.output();

		// Create XML for scheduled task
		let task_xml = format!(
			r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Spacedrive Daemon Background Service</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions>
    <Exec>
      <Command>{}</Command>
      <Arguments>--data-dir "{}"</Arguments>
    </Exec>
  </Actions>
</Task>"#,
			daemon_path.display(),
			data_dir.display()
		);

		// Write XML to temp file
		let temp_dir = std::env::temp_dir();
		let xml_path = temp_dir.join("spacedrive-task.xml");
		let mut file = std::fs::File::create(&xml_path)
			.map_err(|e| format!("Failed to create task XML: {}", e))?;
		file.write_all(task_xml.as_bytes())
			.map_err(|e| format!("Failed to write task XML: {}", e))?;
		drop(file);

		// Create the scheduled task
		let output = std::process::Command::new("schtasks")
			.args(&[
				"/Create",
				"/TN",
				"SpacedriveDaemon",
				"/XML",
				xml_path.to_str().unwrap(),
			])
			.output()
			.map_err(|e| format!("Failed to create scheduled task: {}", e))?;

		// Clean up temp file
		let _ = std::fs::remove_file(&xml_path);

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			tracing::error!("schtasks create failed: {:?}", stderr);
			return Err(format!("Failed to create scheduled task: {}", stderr));
		}

		// Start the task
		let output = std::process::Command::new("schtasks")
			.args(&["/Run", "/TN", "SpacedriveDaemon"])
			.output()
			.map_err(|e| format!("Failed to start scheduled task: {}", e))?;

		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			tracing::error!("schtasks run failed: {:?}", stderr);
			return Err(format!("Failed to start daemon task: {}", stderr));
		}

		// Update daemon state - we no longer own the process
		let mut state = daemon_state.write().await;
		state.started_by_us = false;
		state.daemon_process = None;
		tracing::info!("Updated daemon state: started_by_us = false");
		drop(state);

		// Reset connection pool so it can reconnect to the service-managed daemon
		tracing::info!("Resetting connection pool to reconnect to service daemon");
		app_state.connection_pool.reset().await;

		// Wait for daemon to start and become available
		tracing::info!("Waiting for daemon to become available...");
		for i in 0..30 {
			tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
			if is_daemon_running(&socket_addr).await {
				tracing::info!("Daemon is running after {} attempts", i + 1);
				break;
			}
			if i == 29 {
				return Err("Daemon failed to start after installing service".to_string());
			}
		}

		// Trigger reconnection
		tracing::info!("Triggering reconnection");
		app_state.connection_pool.ensure_connected(&app).await?;

		Ok(())
	}

	#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
	{
		Err("Service installation not supported on this platform".to_string())
	}
}

/// Uninstall daemon service
#[tauri::command]
async fn uninstall_daemon_service() -> Result<(), String> {
	#[cfg(target_os = "macos")]
	{
		let home =
			std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
		let plist_path = std::path::PathBuf::from(&home)
			.join("Library/LaunchAgents/com.spacedrive.daemon.plist");

		if plist_path.exists() {
			// Unload the service
			let _ = std::process::Command::new("launchctl")
				.args(["unload", plist_path.to_str().unwrap()])
				.output();

			std::fs::remove_file(&plist_path)
				.map_err(|e| format!("Failed to remove plist file: {}", e))?;
		}

		Ok(())
	}

	#[cfg(target_os = "linux")]
	{
		let home =
			std::env::var("HOME").map_err(|_| "Could not determine home directory".to_string())?;
		let service_path =
			std::path::PathBuf::from(&home).join(".config/systemd/user/spacedrive-daemon.service");

		if service_path.exists() {
			// Stop and disable the service
			let _ = std::process::Command::new("systemctl")
				.args(&["--user", "stop", "spacedrive-daemon.service"])
				.output();

			let _ = std::process::Command::new("systemctl")
				.args(&["--user", "disable", "spacedrive-daemon.service"])
				.output();

			std::fs::remove_file(&service_path)
				.map_err(|e| format!("Failed to remove service file: {}", e))?;

			let _ = std::process::Command::new("systemctl")
				.args(&["--user", "daemon-reload"])
				.output();
		}

		Ok(())
	}

	#[cfg(target_os = "windows")]
	{
		// Stop the task first
		let _ = std::process::Command::new("schtasks")
			.args(&["/End", "/TN", "SpacedriveDaemon"])
			.output();

		// Delete the scheduled task
		let output = std::process::Command::new("schtasks")
			.args(&["/Delete", "/TN", "SpacedriveDaemon", "/F"])
			.output()
			.map_err(|e| format!("Failed to delete scheduled task: {}", e))?;

		// It's okay if the task doesn't exist
		if !output.status.success() {
			let stderr = String::from_utf8_lossy(&output.stderr);
			// Task not found is okay, other errors should be reported
			if !stderr.contains("cannot find") && !stderr.is_empty() {
				tracing::warn!("schtasks delete warning: {:?}", stderr);
			}
		}

		Ok(())
	}

	#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
	{
		Err("Service installation not supported on this platform".to_string())
	}
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

/// Check if daemon is running by trying to connect and send a ping
async fn is_daemon_running(socket_addr: &str) -> bool {
	use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
	use tokio::net::TcpStream;

	// Try to connect and send a ping request
	let mut stream = match TcpStream::connect(socket_addr).await {
		Ok(s) => s,
		Err(_) => return false,
	};

	// Send a ping request
	let ping_request = serde_json::json!({
		"Ping": null
	});

	let request_line = match serde_json::to_string(&ping_request) {
		Ok(s) => s,
		Err(_) => return false,
	};

	if stream
		.write_all(format!("{}\n", request_line).as_bytes())
		.await
		.is_err()
	{
		return false;
	}

	// Try to read response with a short timeout
	let (reader, _writer) = stream.into_split();
	let mut buf_reader = BufReader::new(reader);
	let mut response_line = String::new();

	// Add a timeout for reading
	match tokio::time::timeout(
		tokio::time::Duration::from_millis(500),
		buf_reader.read_line(&mut response_line),
	)
	.await
	{
		Ok(Ok(_)) if !response_line.is_empty() => {
			tracing::debug!("Daemon responded to ping: {}", response_line.trim());
			true
		}
		_ => {
			tracing::debug!("Daemon did not respond to ping");
			false
		}
	}
}

/// Start the daemon as a background process
async fn start_daemon(
	data_dir: &PathBuf,
	socket_addr: &str,
) -> Result<std::process::Child, String> {
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

	// Wait for daemon to be ready
	for i in 0..30 {
		tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
		if is_daemon_running(socket_addr).await {
			tracing::info!("Daemon ready at {}", socket_addr);
			return Ok(child);
		}
		if i == 10 {
			tracing::warn!("Daemon taking longer than expected to start...");
		}
	}

	Err("Daemon failed to start (connection not available after 3 seconds)".to_string())
}

fn setup_menu(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
	use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

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
	let open_library_item = MenuItemBuilder::with_id("open-library", "Open Library...")
		.accelerator("Cmd+O")
		.build(app)?;

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
		.item(&open_library_item)
		.separator()
		.item(&duplicate_item)
		.separator()
		.item(&rename_item)
		.separator()
		.item(&delete_item)
		.build()?;

	// Edit menu with custom file operations and native text operations
	// Accelerators are handled smartly: native clipboard for text inputs, file ops for explorer
	// IMPORTANT: Keep these always enabled so accelerators work in text inputs
	let cut_item = MenuItemBuilder::with_id("cut", "Cut")
		.accelerator("Cmd+X")
		.enabled(true)
		.build(app)?;
	menu_items_map.insert("cut".to_string(), cut_item.clone());

	let copy_item = MenuItemBuilder::with_id("copy", "Copy")
		.accelerator("Cmd+C")
		.enabled(true)
		.build(app)?;
	menu_items_map.insert("copy".to_string(), copy_item.clone());

	let paste_item = MenuItemBuilder::with_id("paste", "Paste")
		.accelerator("Cmd+V")
		.enabled(true)
		.build(app)?;
	menu_items_map.insert("paste".to_string(), paste_item.clone());

	let edit_menu = SubmenuBuilder::new(app, "Edit")
		.item(&PredefinedMenuItem::undo(app, None)?)
		.item(&PredefinedMenuItem::redo(app, None)?)
		.separator()
		.item(&cut_item)
		.item(&copy_item)
		.item(&paste_item)
		.item(&PredefinedMenuItem::select_all(app, None)?)
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
		.item(&edit_menu)
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
			"open-library" => {
				let app_clone = app_handle.clone();
				tauri::async_runtime::spawn(async move {
					use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

					// Show folder picker dialog
					let folder_path = app_clone
						.dialog()
						.file()
						.set_title("Select Library Folder")
						.set_directory(std::path::PathBuf::from("."))
						.blocking_pick_folder();

					if let Some(path) = folder_path {
						tracing::info!("Selected library path: {:?}", path);

						// Get daemon state
						let daemon_state: tauri::State<Arc<RwLock<DaemonState>>> =
							app_clone.state();
						let state = daemon_state.read().await;

						// Convert FilePath to PathBuf
						let default_path = std::path::PathBuf::from(".");
						let path_buf = path.as_path().unwrap_or(&default_path);

						// Create the JSON-RPC request
						let request = serde_json::json!({
							"jsonrpc": "2.0",
							"id": 1,
							"method": "action:libraries.open.input",
							"params": {
								"path": path_buf.to_string_lossy().to_string()
							}
						});

						drop(state);

						// Send request to daemon using the daemon_request logic
						use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
						use tokio::net::TcpStream;

						let state = daemon_state.read().await;
						let socket_addr = state.socket_addr.clone();
						drop(state);

						match TcpStream::connect(&socket_addr).await {
							Ok(mut stream) => {
								// Send request
								let request_line = match serde_json::to_string(&request) {
									Ok(s) => s,
									Err(e) => {
										tracing::error!("Failed to serialize request: {}", e);
										return;
									}
								};

								if let Err(e) = stream
									.write_all(format!("{}\n", request_line).as_bytes())
									.await
								{
									tracing::error!("Failed to write request: {}", e);
									app_clone
										.dialog()
										.message(format!("Failed to send request to daemon: {}", e))
										.kind(MessageDialogKind::Error)
										.title("Error")
										.blocking_show();
									return;
								}

								// Read response
								let mut reader = BufReader::new(stream);
								let mut response_line = String::new();

								match reader.read_line(&mut response_line).await {
									Ok(_) => {
										match serde_json::from_str::<serde_json::Value>(
											&response_line,
										) {
											Ok(response) => {
												tracing::info!(
													"Library opened successfully: {:?}",
													response
												);
												// Emit event to notify frontend
												if let Err(e) =
													app_clone.emit("library-opened", response)
												{
													tracing::error!(
														"Failed to emit library-opened event: {}",
														e
													);
												}
											}
											Err(e) => {
												tracing::error!("Failed to parse response: {}", e);
												app_clone
													.dialog()
													.message(format!(
														"Failed to open library: {}",
														e
													))
													.kind(MessageDialogKind::Error)
													.title("Error")
													.blocking_show();
											}
										}
									}
									Err(e) => {
										tracing::error!("Failed to read response: {}", e);
										app_clone
											.dialog()
											.message(format!(
												"Failed to read response from daemon: {}",
												e
											))
											.kind(MessageDialogKind::Error)
											.title("Error")
											.blocking_show();
									}
								}
							}
							Err(e) => {
								tracing::error!("Failed to connect to daemon: {}", e);
								app_clone
									.dialog()
									.message(format!("Failed to connect to daemon: {}", e))
									.kind(MessageDialogKind::Error)
									.title("Error")
									.blocking_show();
							}
						}
					}
				});
			}
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
			"duplicate" | "rename" | "delete" => {
				if let Err(e) = app_handle.emit("menu-action", event_id) {
					tracing::error!("Failed to emit menu action: {}", e);
				}
			}
			// Edit menu clipboard actions - emit event for smart handling in frontend
			"cut" | "copy" | "paste" => {
				tracing::info!("[Menu] Clipboard action triggered: {}", event_id);
				// Emit generic clipboard event - frontend will decide if it's a text or file operation
				if let Err(e) = app_handle.emit("clipboard-action", event_id) {
					tracing::error!("Failed to emit clipboard action: {}", e);
				} else {
					tracing::info!(
						"[Menu] Clipboard action event emitted successfully: {}",
						event_id
					);
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
			unsubscribe_from_events,
			cleanup_all_connections,
			get_active_subscriptions,
			update_menu_items,
			get_daemon_status,
			start_daemon_process,
			stop_daemon_process,
			check_daemon_installed,
			install_daemon_service,
			uninstall_daemon_service,
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
			files::get_sidecar_path,
			file_opening::get_apps_for_paths,
			file_opening::open_path_default,
			file_opening::open_path_with_app,
			file_opening::open_paths_with_app,
			keybinds::register_keybind,
			keybinds::unregister_keybind,
			keybinds::get_registered_keybinds
		])
		.setup(|app| {
			// Setup native menu
			if let Err(e) = setup_menu(app.handle()) {
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
				sd_desktop_macos::set_drag_ended_callback(
					move |session_id: &str, was_dropped: bool| {
						tracing::info!(
							"[DRAG] Swift callback: session_id={}, was_dropped={}",
							session_id,
							was_dropped
						);
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
							tracing::debug!(
								"[DRAG] Hiding overlay window from callback: {}",
								overlay_label
							);
							// First hide it immediately
							overlay.hide().ok();

							// Then close it after a short delay to avoid window focus flashing
							let overlay_clone = overlay.clone();
							std::thread::spawn(move || {
								std::thread::sleep(std::time::Duration::from_millis(100));
								overlay_clone.close().ok();
							});
						} else {
							tracing::warn!(
								"[DRAG] Overlay window not found in callback: {}",
								overlay_label
							);
						}
					},
				);
				tracing::info!("Drag ended callback registered");
			}

			// Get data directory (use default Spacedrive location)
			let data_dir =
				sd_core::config::default_data_dir().expect("Failed to get default data directory");

			let socket_addr = "127.0.0.1:6969".to_string();

			// Initialize state immediately (before async operations)
			let daemon_state = Arc::new(RwLock::new(DaemonState {
				started_by_us: false,
				socket_addr: socket_addr.clone(),
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
				connection_pool: Arc::new(DaemonConnectionPool::new(socket_addr.clone())),
				subscription_manager: SubscriptionManager::new(),
			};

			// Clone references needed for validation before managing state (which moves it)
			let app_handle = app.handle().clone();
			let app_state_current_library_id = app_state.current_library_id.clone();
			let daemon_state_clone = daemon_state.clone();
			let data_dir_clone = data_dir.clone();

			app.manage(daemon_state.clone());
			app.manage(app_state);
			app.manage(drag::DragCoordinator::new());
			app.manage(keybinds::KeybindState::new());
			app.manage(file_opening::FileOpeningService::new());

			let _handle = app.handle().clone();

			// Initialize daemon connection in background
			tauri::async_runtime::spawn(async move {
				tracing::info!("Data directory: {:?}", data_dir_clone);
				tracing::info!("Socket address: {:?}", socket_addr);

				// Start HTTP server for serving files/sidecars
				match server::start_server(data_dir_clone.clone()).await {
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

				let (started_by_us, child_process) = if is_daemon_running(&socket_addr).await {
					tracing::info!("Daemon already running, connecting to existing instance");
					(false, None)
				} else {
					tracing::info!("No daemon running, starting new instance");
					match start_daemon(&data_dir_clone, &socket_addr).await {
						Ok(child) => (
							true,
							Some(std::sync::Arc::new(tokio::sync::Mutex::new(Some(child)))),
						),
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

				// Validate persisted library ID in background (non-blocking)
				// If library no longer exists, reset the state
				let app_handle_validate = app_handle.clone();
				let app_state_validate = app_state_current_library_id.clone();
				let daemon_state_validate = daemon_state_clone.clone();
				let data_dir_validate = data_dir_clone.clone();
				tokio::spawn(async move {
					// Wait a bit for daemon to be fully ready
					tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

					if let Err(e) = validate_and_reset_library_if_needed(
						app_handle_validate,
						&app_state_validate,
						&daemon_state_validate,
						&data_dir_validate,
					)
					.await
					{
						tracing::warn!("Failed to validate library: {}", e);
					}
				});
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
