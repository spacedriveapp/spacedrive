#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

use std::{
	collections::HashMap,
	fs,
	path::PathBuf,
	sync::{Arc, Mutex, PoisonError},
	time::Duration,
};

use sd_core::{Node, NodeError};

use sd_fda::DiskAccess;
use serde::{Deserialize, Serialize};
use tauri::{
	api::path, ipc::RemoteDomainAccessScope, window::PlatformWebview, AppHandle, FileDropEvent,
	Manager, WindowEvent,
};
use tauri_plugins::{sd_error_plugin, sd_server_plugin};
use tauri_specta::{collect_events, ts, Event};
use tokio::time::sleep;
use tracing::error;

mod tauri_plugins;

mod theme;

mod file;
mod menu;
mod updater;

#[tauri::command(async)]
#[specta::specta]
async fn app_ready(app_handle: AppHandle) {
	let window = app_handle.get_window("main").unwrap();
	window.show().unwrap();
}

#[tauri::command(async)]
#[specta::specta]
// If this erorrs, we don't have FDA and we need to re-prompt for it
async fn request_fda_macos() {
	DiskAccess::request_fda().expect("Unable to request full disk access");
}

#[tauri::command(async)]
#[specta::specta]
async fn set_menu_bar_item_state(_window: tauri::Window, _id: String, _enabled: bool) {
	#[cfg(target_os = "macos")]
	{
		_window
			.menu_handle()
			.get_item(&_id)
			.set_enabled(_enabled)
			.expect("Unable to modify menu item");
	}
}

#[tauri::command(async)]
#[specta::specta]
async fn reload_webview(app_handle: AppHandle) {
	app_handle
		.get_window("main")
		.expect("Error getting window handle")
		.with_webview(reload_webview_inner)
		.expect("Error while reloading webview");
}

fn reload_webview_inner(webview: PlatformWebview) {
	#[cfg(target_os = "macos")]
	{
		unsafe {
			sd_desktop_macos::reload_webview(&webview.inner().cast());
		}
	}
	#[cfg(target_os = "linux")]
	{
		use webkit2gtk::traits::WebViewExt;

		webview.inner().reload();
	}
	#[cfg(target_os = "windows")]
	unsafe {
		webview
			.controller()
			.CoreWebView2()
			.expect("Unable to get handle on inner webview")
			.Reload()
			.expect("Unable to reload webview");
	}
}

#[tauri::command(async)]
#[specta::specta]
async fn reset_spacedrive(app_handle: AppHandle) {
	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	#[cfg(debug_assertions)]
	let data_dir = data_dir.join("dev");

	fs::remove_dir_all(data_dir).unwrap();

	// TODO: Restarting the app doesn't work in dev (cause Tauri's devserver shutdown) and in prod makes the app go unresponsive until you click in/out on macOS
	// app_handle.restart();

	app_handle.exit(0);
}

#[tauri::command(async)]
#[specta::specta]
async fn refresh_menu_bar(
	_node: tauri::State<'_, Arc<Node>>,
	_app_handle: AppHandle,
) -> Result<(), ()> {
	#[cfg(target_os = "macos")]
	{
		let menu_handles: Vec<tauri::window::MenuHandle> = _app_handle
			.windows()
			.iter()
			.map(|x| x.1.menu_handle())
			.collect();

		let has_library = !_node.libraries.get_all().await.is_empty();

		for menu in menu_handles {
			menu::set_library_locked_menu_items_enabled(menu, has_library);
		}
	}

	Ok(())
}

#[tauri::command(async)]
#[specta::specta]
async fn open_logs_dir(node: tauri::State<'_, Arc<Node>>) -> Result<(), ()> {
	let logs_path = node.data_dir.join("logs");

	#[cfg(target_os = "linux")]
	let open_result = sd_desktop_linux::open_file_path(logs_path);

	#[cfg(not(target_os = "linux"))]
	let open_result = opener::open(logs_path);

	open_result.map_err(|e| {
		error!("Failed to open logs dir: {e:#?}");
	})
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub enum DragAndDropEvent {
	Hovered { paths: Vec<String>, x: f64, y: f64 },
	Dropped { paths: Vec<String>, x: f64, y: f64 },
	Cancelled,
}

#[derive(Default)]
pub struct DragAndDropState {
	windows: HashMap<tauri::Window, tokio::task::JoinHandle<()>>,
}

const CLIENT_ID: &str = "2abb241e-40b8-4517-a3e3-5594375c8fbb";

#[tokio::main]
async fn main() -> tauri::Result<()> {
	#[cfg(target_os = "linux")]
	sd_desktop_linux::normalize_environment();

	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	#[cfg(debug_assertions)]
	let data_dir = data_dir.join("dev");

	// The `_guard` must be assigned to variable for flushing remaining logs on main exit through Drop
	let (_guard, result) = match Node::init_logger(&data_dir) {
		Ok(guard) => (
			Some(guard),
			Node::new(
				data_dir,
				sd_core::Env {
					api_url: "https://app.spacedrive.com".to_string(),
					client_id: CLIENT_ID.to_string(),
				},
			)
			.await,
		),
		Err(err) => (None, Err(NodeError::Logger(err))),
	};

	let app = tauri::Builder::default();

	let (node_router, app) = match result {
		Ok((node, router)) => (Some((node, router)), app),
		Err(err) => {
			error!("Error starting up the node: {err:#?}");
			(None, app.plugin(sd_error_plugin(err)))
		}
	};

	let (node, router) = if let Some((node, router)) = node_router {
		(node, router)
	} else {
		panic!("Unable to get the node or router");
	};

	let app = app
		.plugin(rspc::integrations::tauri::plugin(router, {
			let node = node.clone();
			move || node.clone()
		}))
		.plugin(sd_server_plugin(node.clone()).unwrap()) // TODO: Handle `unwrap`
		.manage(node.clone());

	// macOS expected behavior is for the app to not exit when the main window is closed.
	// Instead, the window is hidden and the dock icon remains so that on user click it should show the window again.
	#[cfg(target_os = "macos")]
	let app = app.on_window_event(|event| {
		if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
			if event.window().label() == "main" {
				AppHandle::hide(&event.window().app_handle()).expect("Window should hide on macOS");
				api.prevent_close();
			}
		}
	});

	let specta_builder = {
		let specta_builder = ts::builder()
			.events(collect_events![DragAndDropEvent])
			.commands(tauri_specta::collect_commands![
				app_ready,
				reset_spacedrive,
				open_logs_dir,
				refresh_menu_bar,
				reload_webview,
				set_menu_bar_item_state,
				request_fda_macos,
				file::open_file_paths,
				file::open_ephemeral_files,
				file::get_file_path_open_with_apps,
				file::get_ephemeral_files_open_with_apps,
				file::open_file_path_with,
				file::open_ephemeral_file_with,
				file::reveal_items,
				theme::lock_app_theme,
				// TODO: move to plugin w/tauri-specta
				updater::check_for_update,
				updater::install_update
			])
			// .events(tauri_specta::collect_events![])
			.config(specta::ts::ExportConfig::default().formatter(specta::ts::formatter::prettier));

		#[cfg(debug_assertions)]
		let specta_builder = specta_builder.path("../src/commands.ts");

		specta_builder.into_plugin()
	};

	let file_drop_status = Arc::new(Mutex::new(DragAndDropState::default()));
	let app = app
		.plugin(updater::plugin())
		// .plugin(tauri_plugin_window_state::Builder::default().build()) // TODO: Fix this
		.plugin(specta_builder)
		.setup(move |app| {
			let app = app.handle();

			app.windows().iter().for_each(|(_, window)| {
				tokio::spawn({
					let window = window.clone();
					async move {
						sleep(Duration::from_secs(3)).await;
						if !window.is_visible().unwrap_or(true) {
							// This happens if the JS bundle crashes and hence doesn't send ready event.
							println!(
								"Window did not emit `app_ready` event fast enough. Showing window..."
							);
							window.show().expect("Main window should show");
						}
					}
				});

				#[cfg(target_os = "windows")]
				window.set_decorations(true).unwrap();

				#[cfg(target_os = "macos")]
				{
					use sd_desktop_macos::{blur_window_background, set_titlebar_style};

					let nswindow = window.ns_window().unwrap();

					unsafe { set_titlebar_style(&nswindow, true) };
					unsafe { blur_window_background(&nswindow) };

					tokio::spawn({
						let libraries = node.libraries.clone();
						let menu_handle = window.menu_handle();
						async move {
							if libraries.get_all().await.is_empty() {
								menu::set_library_locked_menu_items_enabled(menu_handle, false);
							}
						}
					});
				}
			});

			// Configure IPC for custom protocol
			app.ipc_scope().configure_remote_access(
				RemoteDomainAccessScope::new("localhost")
					.allow_on_scheme("spacedrive")
					.add_window("main"),
			);

			Ok(())
		})
		.on_menu_event(menu::handle_menu_event)
		.on_window_event(move |event| match event.event() {
			WindowEvent::FileDrop(drop) => {
				let window = event.window();
				let mut file_drop_status = file_drop_status
					.lock()
					.unwrap_or_else(PoisonError::into_inner);

				match drop {
					FileDropEvent::Hovered { paths, .. } => {
						// Look this shouldn't happen but let's be sure we don't leak threads.
						if file_drop_status.windows.contains_key(window) {
							return;
						}

						// We setup a thread to keep emitting the updated position of the cursor
						// It will be killed when the `FileDropEvent` is finished or cancelled.
						let paths = paths.clone();
						file_drop_status.windows.insert(window.clone(), {
							let window = window.clone();
							tokio::spawn(async move {
								loop {
									let window_pos = window.outer_position().unwrap();
									let cursor_pos = window.cursor_position().unwrap();
									let (x, y) = (
										cursor_pos.x - window_pos.x as f64,
										cursor_pos.y - window_pos.y as f64,
									);

									DragAndDropEvent::Hovered {
										paths: paths
											.iter()
											.filter_map(|x| x.to_str().map(|x| x.to_string()))
											.collect(),
										x,
										y,
									}
									.emit(&window)
									.ok();

									sleep(Duration::from_millis(250)).await;
								}
							})
						});
					}
					FileDropEvent::Dropped { paths, position } => {
						if let Some(handle) = file_drop_status.windows.remove(&window) {
							handle.abort();
						}

						DragAndDropEvent::Dropped {
							paths: paths
								.iter()
								.filter_map(|x| x.to_str().map(|x| x.to_string()))
								.collect(),
							x: position.x,
							y: position.y,
						}
						.emit(&window)
						.ok();
					}
					FileDropEvent::Cancelled => {
						if let Some(handle) = file_drop_status.windows.remove(&window) {
							handle.abort();
						}

						DragAndDropEvent::Cancelled.emit(&window).ok();
					}
					_ => unreachable!(),
				}
			}
			WindowEvent::Resized(_) => {
				let (_state, command) = if event
					.window()
					.is_fullscreen()
					.expect("Can't get fullscreen state")
				{
					(true, "window_fullscreened")
				} else {
					(false, "window_not_fullscreened")
				};

				event
					.window()
					.emit("keybind", command)
					.expect("Unable to emit window event");

				#[cfg(target_os = "macos")]
				{
					let nswindow = event.window().ns_window().unwrap();
					unsafe { sd_desktop_macos::set_titlebar_style(&nswindow, _state) };
				}
			}
			_ => {}
		})
		.menu(menu::get_menu())
		.manage(updater::State::default())
		.build(tauri::generate_context!())?;

	app.run(|_, _| {});
	Ok(())
}
