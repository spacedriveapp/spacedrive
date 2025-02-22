use base64::{engine::general_purpose::STANDARD, Engine as _};
use drag::{DragItem, Image, Options};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{ipc::Channel, Manager, PhysicalPosition, State, WebviewWindow};

// DragState wraps a thread-safe boolean flag to track drag operation status
#[derive(Clone)]
pub struct DragState(pub Arc<Mutex<bool>>);

// Default implementation for DragState initializes with false
impl Default for DragState {
	fn default() -> Self {
		Self(Arc::new(Mutex::new(false)))
	}
}

// Enum to represent the result of a drag operation (serializable for IPC)
#[derive(Serialize, Deserialize, Type, Clone)]
pub enum WrappedDragResult {
	Dropped,
	Cancel,
}

// Structure to hold cursor position coordinates (serializable for IPC)
#[derive(Serialize, Deserialize, Type, Clone)]
pub struct WrappedCursorPosition {
	x: i32,
	y: i32,
}

// Combined structure for drag operation results (serializable for IPC)
#[derive(Serialize, Deserialize, Type, Clone)]
pub struct CallbackResult {
	result: WrappedDragResult,
	#[serde(rename = "cursorPos")]
	cursor_pos: WrappedCursorPosition,
}

// Conversion implementations for drag-rs types to our wrapped types
impl From<drag::DragResult> for WrappedDragResult {
	fn from(result: drag::DragResult) -> Self {
		match result {
			drag::DragResult::Dropped => WrappedDragResult::Dropped,
			drag::DragResult::Cancel => WrappedDragResult::Cancel,
		}
	}
}

impl From<drag::CursorPosition> for WrappedCursorPosition {
	fn from(pos: drag::CursorPosition) -> Self {
		WrappedCursorPosition { x: pos.x, y: pos.y }
	}
}

// Global flag to track if position tracking is active
static TRACKING: AtomicBool = AtomicBool::new(false);

/// Initiates a drag and drop operation with cursor position tracking
///
/// # Arguments
/// * `window` - The Tauri window instance
/// * `_state` - Current drag state (unused)
/// * `files` - Vector of file paths to be dragged
/// * `image` - Base64 encoded image to be used as drag icon
/// * `on_event` - Channel for communicating drag operation events back to the frontend
#[tauri::command(async)]
#[specta::specta]
#[cfg(not(target_os = "linux"))]
pub async fn start_drag(
	window: WebviewWindow,
	_state: State<'_, DragState>,
	files: Vec<String>,
	image: String,
	on_event: Channel<CallbackResult>,
) -> Result<(), String> {
	// Check if image string is base64 encoded
	let icon_path = if image.starts_with("data:image/") {
		image
	} else {
		// If not, assume it's a file path and convert to base64
		let icon_data = std::fs::read(&image).map_err(|e| e.to_string())?;
		format!("data:image/png;base64,{}", STANDARD.encode(icon_data))
	};

	// Convert the base64 string to a vec<u8>
	let base64_str = icon_path.split(",").last().unwrap();
	let image_raw = STANDARD.decode(base64_str).unwrap();

	// Fast atomic swap for tracking state
	match TRACKING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
		Ok(_) => {
			println!("Starting position tracking");
		}
		Err(_) => {
			// If already tracking, stop previous instance quickly
			TRACKING.store(false, Ordering::SeqCst);
			tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
			TRACKING.store(true, Ordering::SeqCst);
			println!("Restarting position tracking");
		}
	}

	// Pre-allocate resources before spawning task
	let window_handle = Arc::new(window);
	let app_handle = window_handle.app_handle();

	// Initialize control flags
	let cancel_flag = Arc::new(AtomicBool::new(false));
	let is_completed = Arc::new(AtomicBool::new(false));

	// Prepare resources once with minimal cloning
	let tracking_resources = Arc::new((files.clone(), icon_path.clone(), Arc::new(on_event)));

	println!("Starting position tracking");

	// Get handles for window and app management
	let window_clone = window_handle.clone();
	let app_handle_owned = app_handle.to_owned();
	let window_owned = window_clone.to_owned();

	// Control flags for operation state
	let is_completed_clone = is_completed.clone();

	// Spawn background task for cursor tracking
	tokio::spawn(async move {
		// Initialize tracking state
		let mut last_position = (0.0, 0.0);
		let mut last_message_time = Instant::now();
		let threshold = 1.0; // Minimum movement threshold
		let message_debounce = Duration::from_millis(32); // State update interval
		let mut was_inside = false;

		// Main tracking loop
		while TRACKING.load(Ordering::SeqCst) && !is_completed.load(Ordering::SeqCst) {
			let window_for_check = window_owned.clone();
			// Skip if window is not focused
			if !window_for_check.is_focused().unwrap_or(false) {
				tokio::time::sleep(tokio::time::Duration::from_millis(8)).await;
				continue;
			}

			// Get current cursor and window positions
			if let (Ok(cursor_position), Ok(window_position), Ok(window_size)) = (
				window_for_check.cursor_position(),
				window_for_check.outer_position(),
				window_for_check.inner_size(),
			) {
				// Calculate cursor position relative to window
				let relative_position = PhysicalPosition::new(
					cursor_position.x - window_position.x as f64,
					cursor_position.y - window_position.y as f64,
				);

				// Check if cursor is inside window boundaries
				let is_inside = relative_position.x >= 0.0
					&& relative_position.y >= 0.0
					&& relative_position.x <= window_size.width as f64
					&& relative_position.y <= window_size.height as f64;

				// Process state changes if cursor moved enough
				if is_inside != was_inside
					&& ((relative_position.x - last_position.0).abs() > threshold
						|| (relative_position.y - last_position.1).abs() > threshold)
				{
					let now = Instant::now();
					if now.duration_since(last_message_time) >= message_debounce {
						// Prepare resources for drag operation
						let files_for_drag = tracking_resources.0.clone();
						let icon_path_for_drag = tracking_resources.1.clone();
						let on_event_for_drag = tracking_resources.2.clone();
						let is_completed = is_completed_clone.clone();
						let cancel_flag_clone = cancel_flag.clone();
						let window_for_drag = window_owned.clone();
						let image_raw_for_drag = image_raw.clone();

						// Execute drag operation on main thread
						app_handle_owned
							.run_on_main_thread(move || {
								if !is_inside {
									println!("Starting drag operation");
									// Create drag items
									let paths: Vec<PathBuf> =
										files_for_drag.iter().map(PathBuf::from).collect();
									let item = DragItem::Files(paths);
									let preview_icon = Image::Raw(image_raw_for_drag.clone());

									// Start the drag operation
									if let Ok(session) = drag::start_drag(
										&window_for_drag,
										item,
										preview_icon,
										move |result, cursor_pos| {
											println!("Drag operation completed");
											// Send result back to frontend
											let _ = on_event_for_drag.send(CallbackResult {
												result: result.into(),
												cursor_pos: cursor_pos.into(),
											});
											// Mark operation as completed
											is_completed.store(true, Ordering::SeqCst);
											TRACKING.store(false, Ordering::SeqCst);
										},
										Options {
											skip_animatation_on_cancel_or_failure: false,
											mode: drag::DragMode::Move,
										},
									) {
										println!("Drag operation started");
										// Store drag session for cancellation
										// *drag_session_clone.lock().unwrap() = Some(session);
									}
								} else {
									println!("Cursor returned to window");
									cancel_flag_clone.store(true, Ordering::SeqCst);
									// We have this for now, but technically, it doesn't do anything.
									// I'm still trying to figure out how to cancel mid-drag without the user having to cancel the dragging on the frontend too.
									// - @Rocky43007
								}
							})
							.unwrap_or_default();

						// Update tracking state
						last_message_time = now;
						was_inside = is_inside;
						last_position = (relative_position.x, relative_position.y);
					}
				}
			}

			// Prevent excessive CPU usage
			tokio::time::sleep(tokio::time::Duration::from_millis(8)).await;
		}

		println!("Tracking instance stopped");
	});

	Ok(())
}

// /// Initiates a drag and drop operation with cursor position tracking - WIP
// ///
// /// # Arguments
// /// * `window` - The Tauri window instance
// /// * `_state` - Current drag state (unused)
// /// * `files` - Vector of file paths to be dragged
// /// * `image` - Base64 encoded image to be used as drag icon
// /// * `on_event` - Channel for communicating drag operation events back to the frontend
// #[tauri::command(async)]
// #[specta::specta]
// #[cfg(target_os = "linux")]
// pub async fn start_drag(
// 	window: WebviewWindow,
// 	_state: State<'_, DragState>,
// 	files: Vec<String>,
// 	image: String,
// 	on_event: Channel<CallbackResult>,
// ) -> Result<(), String> {
// 	use drag::{CursorPosition, DragResult};
// 	use tao::platform::unix::WindowExtUnix;

// 	// Convert file paths to PathBuf
// 	let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();

// 	// Handle preview image
// 	let preview_icon = if image.starts_with("data:image/") {
// 		let base64_str = image.split(",").last().unwrap();
// 		let image_raw = STANDARD.decode(base64_str).unwrap();
// 		Image::Raw(image_raw)
// 	} else {
// 		Image::File(PathBuf::from(image))
// 	};

// 	// Get main thread handle
// 	let app_handle = window.app_handle();
// 	let window_clone = window.clone();

// 	app_handle
// 		.run_on_main_thread(move || {
// 			// Get GTK window handle
// 			let gtk_window = window_clone.gtk_window().expect("Failed to get GTK window");
// 			let item = DragItem::Files(paths);
// 			println!("Starting drag operation");

// 			// Start drag operation
// 			let _ = drag::start_drag(
// 				&gtk_window,
// 				item,
// 				preview_icon,
// 				move |result, cursor_pos| {
// 					println!("Drag operation completed");
// 					println!("Result: {:?}", result);
// 					println!("Cursor position: {:?}", cursor_pos);
// 					let _ = on_event.send(CallbackResult {
// 						result: result.into(),
// 						cursor_pos: cursor_pos.into(),
// 					});
// 				},
// 				Options {
// 					skip_animatation_on_cancel_or_failure: false,
// 					mode: drag::DragMode::Move,
// 				},
// 			);
// 		})
// 		.unwrap_or_default();

// 	Ok(())
// }

#[tauri::command(async)]
#[specta::specta]
#[cfg(target_os = "linux")]
pub async fn start_drag(
	_window: WebviewWindow,
	_state: State<'_, DragState>,
	_files: Vec<String>,
	_image: String,
	_on_event: Channel<CallbackResult>,
) -> Result<(), String> {
	Err("Drag and drop is not supported on Linux".to_string())
}

/// Stops the cursor position tracking for drag operations
#[tauri::command(async)]
#[specta::specta]
pub async fn stop_drag() {
	TRACKING.store(false, Ordering::SeqCst);
}
