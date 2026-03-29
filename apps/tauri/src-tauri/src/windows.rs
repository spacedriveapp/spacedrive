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
	Spacebot,

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
	VoiceOverlay,

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
			Self::Spacebot => "spacebot".to_string(),
			Self::Inspector { item_id } => {
				format!("inspector-{}", item_id.as_deref().unwrap_or("floating"))
			}
			Self::QuickPreview { file_id } => format!("quick-preview-{}", file_id),
			Self::TagAssignment => "tag-assignment".to_string(),
			Self::SearchOverlay => "search-overlay".to_string(),
			Self::FloatingControls => "floating-controls".to_string(),
			Self::VoiceOverlay => "voice-overlay".to_string(),
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

			Self::Spacebot => create_window(
				app,
				&label,
				"/spacebot",
				"Spacebot",
				(1200.0, 800.0),
				(800.0, 600.0),
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

			Self::VoiceOverlay => {
				let window =
					WebviewWindowBuilder::new(app, label, WebviewUrl::App("/voice-overlay".into()))
						.title("Voice Overlay")
						.inner_size(520.0, 112.0)
						.resizable(false)
						.decorations(false)
						.shadow(false)
						.transparent(true)
						.always_on_top(true)
						.skip_taskbar(true)
						.visible(false)
						.build()
						.map_err(|e| format!("Failed to create voice overlay: {}", e))?;

				position_overlay_window(&window, 520.0, 112.0)?;

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

				#[cfg(target_os = "windows")]
				apply_dark_titlebar(&window);

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

	// macOS: Hide titlebar but keep traffic lights (like main window)
	#[cfg(target_os = "macos")]
	{
		builder = builder.hidden_title(true);
	}

	// Enable DevTools in dev mode
	#[cfg(debug_assertions)]
	{
		builder = builder.devtools(true);
	}

	let window = builder
		.build()
		.map_err(|e| format!("Failed to create window: {}", e))?;

	// Windows: force dark titlebar + override accent color
	#[cfg(target_os = "windows")]
	apply_dark_titlebar(&window);

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

fn position_overlay_window(window: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
	use tauri::{PhysicalPosition, Position};

	let monitor = window
		.current_monitor()
		.map_err(|e| e.to_string())?
		.ok_or("No monitor found")?;

	let monitor_size = monitor.size();
	let monitor_position = monitor.position();
	let scale_factor = window.scale_factor().map_err(|e| e.to_string())?;

	let physical_width = (width * scale_factor).round() as i32;
	let physical_height = (height * scale_factor).round() as i32;
	let bottom_margin = (24.0 * scale_factor).round() as i32;

	let x = monitor_position.x + (monitor_size.width as i32 - physical_width) / 2;
	let y = monitor_position.y + monitor_size.height as i32 - physical_height - bottom_margin;

	window
		.set_position(Position::Physical(PhysicalPosition::new(x, y)))
		.map_err(|e| e.to_string())
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

pub fn toggle_voice_overlay_internal(app: AppHandle) -> Result<(), String> {
	let window = SpacedriveWindow::VoiceOverlay;
	let label = window.label();

	if let Some(existing) = app.get_webview_window(&label) {
		existing.close().map_err(|e| e.to_string())?;
		return Ok(());
	}

	tauri::async_runtime::spawn(async move {
		if let Err(error) = window.show(&app).await {
			tracing::warn!(?error, "Failed to open voice overlay window");
		}
	});

	Ok(())
}

#[tauri::command]
pub async fn toggle_voice_overlay(app: AppHandle) -> Result<(), String> {
	toggle_voice_overlay_internal(app)
}

#[tauri::command]
pub async fn resize_overlay_window(
	app: AppHandle,
	label: String,
	width: f64,
	height: f64,
) -> Result<(), String> {
	use tauri::{LogicalSize, Size};

	let window = app
		.get_webview_window(&label)
		.ok_or("Overlay window not found")?;

	window
		.set_size(Size::Logical(LogicalSize::new(width, height)))
		.map_err(|e| e.to_string())?;

	position_overlay_window(&window, width, height)?;

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

/// Apply dark titlebar on Windows using DWM API.
///
/// Sets both `DWMWA_USE_IMMERSIVE_DARK_MODE` (dark window chrome) and
/// `DWMWA_CAPTION_COLOR` (explicit titlebar color) to override the user's
/// Windows accent color setting which would otherwise tint the titlebar.
#[cfg(target_os = "windows")]
pub fn apply_dark_titlebar_pub(window: &WebviewWindow) {
	apply_dark_titlebar(window);
}

#[cfg(target_os = "windows")]
fn apply_dark_titlebar(window: &WebviewWindow) {
	#[allow(non_snake_case)]
	mod dwm {
		// DWM attribute constants
		pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
		pub const DWMWA_CAPTION_COLOR: u32 = 35;
		pub const DWMWA_BORDER_COLOR: u32 = 34;

		extern "system" {
			pub fn DwmSetWindowAttribute(
				hwnd: isize,
				attr: u32,
				value: *const std::ffi::c_void,
				size: u32,
			) -> i32;
		}
	}

	let Ok(hwnd) = window.hwnd() else {
		tracing::warn!("Failed to get HWND for dark titlebar");
		return;
	};
	let hwnd = hwnd.0 as isize;

	unsafe {
		let set_attr =
			|attr: u32, value: *const std::ffi::c_void, size: u32, name: &'static str| {
				let hr = dwm::DwmSetWindowAttribute(hwnd, attr, value, size);
				if hr < 0 {
					tracing::warn!(attribute = name, hr, "Failed to apply DWM window attribute");
				}
			};

		// Enable immersive dark mode (dark close/minimize/maximize icons)
		let dark_mode: i32 = 1;
		set_attr(
			dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
			&dark_mode as *const _ as *const std::ffi::c_void,
			std::mem::size_of::<i32>() as u32,
			"DWMWA_USE_IMMERSIVE_DARK_MODE",
		);

		// Force caption color to dark gray — overrides user's accent color
		// COLORREF format is 0x00BBGGRR
		let caption_color: u32 = 0x00_1E_1E_1E; // #1E1E1E in BGR
		set_attr(
			dwm::DWMWA_CAPTION_COLOR,
			&caption_color as *const _ as *const std::ffi::c_void,
			std::mem::size_of::<u32>() as u32,
			"DWMWA_CAPTION_COLOR",
		);

		// Match border color to caption
		set_attr(
			dwm::DWMWA_BORDER_COLOR,
			&caption_color as *const _ as *const std::ffi::c_void,
			std::mem::size_of::<u32>() as u32,
			"DWMWA_BORDER_COLOR",
		);
	}
}
