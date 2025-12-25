use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Window types in Spacedrive
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SpacedriveWindow {
	/// Main application window
	Main,

	/// Multi-instance windows
	Explorer {
		library_id: String,
		path: String,
	},
	MediaViewer {
		file_id: String,
	},

	/// Single-instance auxiliary windows
	Settings {
		page: Option<String>,
	},
	JobManager,
	DeviceDiscovery,

	/// Floating panels (always on top)
	Inspector {
		item_id: Option<String>,
	},
	QuickPreview {
		file_id: String,
	},

	/// Overlays (transparent, fullscreen)
	TagAssignment,
	SearchOverlay,

	/// Floating controls (small, always on top)
	FloatingControls,

	/// Drag demo window
	DragDemo,

	/// Spacedrop window
	Spacedrop,

	/// Drag overlay (cursor-tracking preview during drag operations)
	DragOverlay {
		session_id: String,
	},

	/// Context menu (native transparent window at cursor)
	ContextMenu {
		context_id: String,
	},
}

impl SpacedriveWindow {
	/// Generate unique label for this window
	pub fn label(&self) -> String {
		match self {
			Self::Main => "main".to_string(),
			Self::Explorer { library_id, path } => {
				format!("explorer-{}-{}", library_id, hash_string(path))
			}
			Self::MediaViewer { file_id } => format!("media-viewer-{}", file_id),
			Self::Settings { page } => format!("settings-{}", page.as_deref().unwrap_or("general")),
			Self::JobManager => "job-manager".to_string(),
			Self::DeviceDiscovery => "device-discovery".to_string(),
			Self::Inspector { item_id } => {
				format!("inspector-{}", item_id.as_deref().unwrap_or("floating"))
			}
			Self::QuickPreview { file_id } => format!("quick-preview-{}", file_id),
			Self::TagAssignment => "tag-assignment".to_string(),
			Self::SearchOverlay => "search-overlay".to_string(),
			Self::FloatingControls => "floating-controls".to_string(),
			Self::DragDemo => "drag-demo".to_string(),
			Self::Spacedrop => "spacedrop".to_string(),
			Self::DragOverlay { session_id } => format!("drag-overlay-{}", session_id),
			Self::ContextMenu { context_id } => format!("context-menu-{}", context_id),
		}
	}

	/// Get existing window if it exists
	pub fn get(&self, app: &AppHandle) -> Option<WebviewWindow> {
		app.get_webview_window(&self.label())
	}

	/// Show or focus this window
	pub async fn show(&self, app: &AppHandle) -> Result<WebviewWindow, String> {
		// If window already exists, just focus it
		if let Some(window) = self.get(app) {
			window.set_focus().ok();
			return Ok(window);
		}

		// Create new window based on type
		self.create(app).await
	}

	/// Create a new window
	async fn create(&self, app: &AppHandle) -> Result<WebviewWindow, String> {
		let label = self.label();

		match self {
			Self::Main => {
				create_window(
					app,
					&label,
					"/",
					"Spacedrive",
					(1400.0, 750.0),
					(768.0, 500.0),
					true,  // decorations
					false, // always_on_top
					false, // transparent
				)
			}

			Self::Explorer { library_id, path } => {
				let url = format!("/explorer/{}/{}", library_id, path);
				create_window(
					app,
					&label,
					&url,
					"Explorer",
					(1200.0, 800.0),
					(800.0, 600.0),
					true,
					false,
					false,
				)
			}

			Self::Settings { page } => {
				let url = format!("/settings/{}", page.as_deref().unwrap_or(""));
				create_window(
					app,
					&label,
					&url,
					"Settings",
					(900.0, 700.0),
					(600.0, 400.0),
					true,
					false,
					false,
				)
			}

			Self::Inspector { item_id } => {
				let url = format!("/inspector/{}", item_id.as_deref().unwrap_or(""));
				let window = create_window(
					app,
					&label,
					&url,
					"Inspector",
					(400.0, 600.0),
					(320.0, 400.0),
					true,
					true, // always on top
					false,
				)?;

				// Listen for window close to notify main window
				let app_handle = app.clone();
				window.on_window_event(move |event| {
					if let tauri::WindowEvent::CloseRequested { .. } = event {
						app_handle.emit("inspector-window-closed", ()).ok();
					}
				});

				Ok(window)
			}

			Self::MediaViewer { file_id } => {
				let url = format!("/media-viewer/{}", file_id);
				create_window(
					app,
					&label,
					&url,
					"Media Viewer",
					(1200.0, 800.0),
					(800.0, 600.0),
					true,
					false,
					false,
				)
			}

			Self::JobManager => {
				create_window(
					app,
					&label,
					"/job-manager",
					"Job Manager",
					(600.0, 500.0),
					(400.0, 300.0),
					true,
					true, // floating
					false,
				)
			}

			Self::DeviceDiscovery => create_window(
				app,
				&label,
				"/devices",
				"Devices",
				(800.0, 600.0),
				(600.0, 400.0),
				true,
				false,
				false,
			),

			Self::QuickPreview { file_id } => {
				let url = format!("/quick-preview/{}", file_id);
				create_window(
					app,
					&label,
					&url,
					"Quick Look",
					(800.0, 600.0),
					(600.0, 400.0),
					true,
					true, // floating
					false,
				)
			}

			Self::TagAssignment => {
				create_window(
					app,
					&label,
					"/tag-assignment",
					"Tag Assignment",
					(0.0, 0.0),
					(0.0, 0.0),
					false, // no decorations
					true,  // always on top
					true,  // transparent
				)
			}

			Self::SearchOverlay => {
				create_window(
					app,
					&label,
					"/search",
					"Search",
					(800.0, 600.0),
					(600.0, 400.0),
					false, // no decorations
					true,  // always on top
					true,  // transparent
				)
			}

			Self::FloatingControls => {
				// Small floating control panel like Cap's recording controls
				let window = WebviewWindowBuilder::new(
					app,
					label,
					WebviewUrl::App("/floating-controls".into()),
				)
				.title("Controls")
				.inner_size(200.0, 80.0)
				.resizable(false)
				.decorations(false)
				.transparent(true)
				.always_on_top(true)
				.skip_taskbar(true)
				.build()
				.map_err(|e| format!("Failed to create window: {}", e))?;

				// Position at bottom center of screen
				#[cfg(target_os = "macos")]
				{
					use tauri::Position;
					// Get screen size and position window
					if let Ok(Some(monitor)) = window.current_monitor() {
						let size = monitor.size();
						// Bottom center, 40px from bottom
						window
							.set_position(Position::Physical(tauri::PhysicalPosition {
								x: (size.width as i32) / 2 - 100,
								y: (size.height as i32) - 120,
							}))
							.ok();
					}
				}

				window.show().ok();
				Ok(window)
			}

			Self::DragDemo => create_window(
				app,
				&label,
				"/drag-demo",
				"Drag Demo",
				(600.0, 400.0),
				(400.0, 300.0),
				true,
				false,
				false,
			),

			Self::Spacedrop => create_window(
				app,
				&label,
				"/spacedrop",
				"Spacedrop",
				(800.0, 600.0),
				(600.0, 400.0),
				true,
				false,
				false,
			),

			Self::DragOverlay { session_id } => {
				let url = format!("/drag-overlay?session={}", session_id);
				let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
					.title("")
					.inner_size(200.0, 150.0)
					.resizable(false)
					.decorations(false)
					.transparent(true)
					.always_on_top(true)
					.skip_taskbar(true)
					.visible(false)
					.build()
					.map_err(|e| format!("Failed to create drag overlay: {}", e))?;

				// macOS-specific window configuration handled by Tauri

				Ok(window)
			}

			Self::ContextMenu { context_id } => {
				let url = format!("/contextmenu?context={}", context_id);
				let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
					.title("Context Menu Debug")
					.inner_size(250.0, 300.0) // Initial size, will be adjusted by content
					.resizable(false)
					.decorations(true) // TEMP: Show decorations for debugging
					.transparent(false) // TEMP: Not transparent for debugging
					.always_on_top(true)
					.skip_taskbar(false) // TEMP: Show in taskbar for debugging
					.visible(true) // TEMP: Make visible immediately for debugging
					.focused(true)
					.build()
					.map_err(|e| format!("Failed to create context menu: {}", e))?;

				Ok(window)
			}
		}
	}
}

/// Helper to create a window with common configuration
#[allow(clippy::too_many_arguments)]
fn create_window(
	app: &AppHandle,
	label: &str,
	url: &str,
	title: &str,
	size: (f64, f64),
	min_size: (f64, f64),
	decorations: bool,
	always_on_top: bool,
	transparent: bool,
) -> Result<WebviewWindow, String> {
	let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
		.title(title)
		.inner_size(size.0, size.1)
		.min_inner_size(min_size.0, min_size.1)
		.resizable(true)
		.decorations(decorations)
		.transparent(transparent)
		.always_on_top(always_on_top)
		.center();

	// Enable DevTools in dev mode
	#[cfg(debug_assertions)]
	{
		builder = builder.devtools(true);
	}

	let window = builder
		.build()
		.map_err(|e| format!("Failed to create window: {}", e))?;

	window.show().ok();
	window.set_focus().ok();

	Ok(window)
}

/// Simple hash for generating window IDs
fn hash_string(s: &str) -> String {
	use std::collections::hash_map::DefaultHasher;
	use std::hash::{Hash, Hasher};

	let mut hasher = DefaultHasher::new();
	s.hash(&mut hasher);
	format!("{:x}", hasher.finish())
}

/// Tauri command to show a window
#[tauri::command]
pub async fn show_window(app: AppHandle, window: SpacedriveWindow) -> Result<String, String> {
	let label = window.label();
	window.show(&app).await?;
	Ok(label)
}

/// Tauri command to close a window
#[tauri::command]
pub async fn close_window(app: AppHandle, label: String) -> Result<(), String> {
	if let Some(window) = app.get_webview_window(&label) {
		window.close().map_err(|e| e.to_string())?;
	}
	Ok(())
}

/// Apply macOS window styling to current window (called from frontend when ready)
#[tauri::command]
pub fn apply_macos_styling(app: AppHandle) -> Result<(), String> {
	#[cfg(target_os = "macos")]
	{
		let window = app
			.get_webview_window(
				&app.webview_windows()
					.keys()
					.last()
					.ok_or("No windows found")?
					.clone(),
			)
			.ok_or("Could not get current window")?;

		match window.ns_window() {
			Ok(ns_window) => unsafe {
				sd_desktop_macos::set_titlebar_style(&ns_window, false);
				Ok(())
			},
			Err(e) => Err(format!("Could not get NSWindow: {}", e)),
		}
	}

	#[cfg(not(target_os = "macos"))]
	Ok(())
}

/// Tauri command to list all open windows
#[tauri::command]
pub async fn list_windows(app: AppHandle) -> Result<Vec<String>, String> {
	Ok(app.webview_windows().into_keys().collect())
}

/// Tauri command to position and show context menu with screen boundary detection
#[tauri::command]
pub async fn position_context_menu(
	app: AppHandle,
	label: String,
	x: f64,
	y: f64,
	menu_width: f64,
	menu_height: f64,
) -> Result<(), String> {
	let window = app
		.get_webview_window(&label)
		.ok_or("Context menu window not found")?;

	use tauri::{PhysicalPosition, Position};

	// Get current monitor
	let monitor = window
		.current_monitor()
		.map_err(|e| e.to_string())?
		.ok_or("No monitor found")?;

	let monitor_size = monitor.size();
	let monitor_pos = monitor.position();

	// Calculate final position with screen boundary clamping
	let final_x = ((x as i32) + monitor_pos.x)
		.min(monitor_pos.x + monitor_size.width as i32 - menu_width as i32)
		.max(monitor_pos.x);

	let final_y = ((y as i32) + monitor_pos.y)
		.min(monitor_pos.y + monitor_size.height as i32 - menu_height as i32)
		.max(monitor_pos.y);

	// Position window
	window
		.set_position(Position::Physical(PhysicalPosition::new(final_x, final_y)))
		.map_err(|e| e.to_string())?;

	// Show and focus
	window.show().map_err(|e| e.to_string())?;
	window.set_focus().map_err(|e| e.to_string())?;

	Ok(())
}
