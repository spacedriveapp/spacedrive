#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::{fs, path::PathBuf, process::Command, sync::Arc, time::Duration};

use menu::{set_enabled, MenuEvent};
use sd_core::{Node, NodeError};

use sd_fda::DiskAccess;
use serde::{Deserialize, Serialize};
use specta_typescript::Typescript;
use tauri::{async_runtime::block_on, webview::PlatformWebview, AppHandle, Manager, WindowEvent};
use tauri::{Emitter, Listener};
use tauri_plugins::{sd_error_plugin, sd_server_plugin};
use tauri_specta::{collect_events, Builder};
use tokio::task::block_in_place;
use tokio::time::sleep;
use tracing::{debug, error};

mod file;
mod menu;
mod tauri_plugins;
mod theme;
mod updater;

#[tauri::command(async)]
#[specta::specta]
async fn app_ready(app_handle: AppHandle) {
	let window = app_handle.get_webview_window("main").unwrap();
	window.show().unwrap();
}

#[tauri::command(async)]
#[specta::specta]
// If this errors, we don't have FDA and we need to re-prompt for it
async fn request_fda_macos() {
	DiskAccess::request_fda().expect("Unable to request full disk access");
}

#[tauri::command(async)]
#[specta::specta]
async fn set_menu_bar_item_state(window: tauri::Window, event: MenuEvent, enabled: bool) {
	let menu = window
		.menu()
		.expect("unable to get menu for current window");

	set_enabled(&menu, event, enabled);
}

#[tauri::command(async)]
#[specta::specta]
async fn reload_webview(app_handle: AppHandle) {
	app_handle
		.get_webview_window("main")
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
		use webkit2gtk::WebViewExt;

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
	let data_dir = app_handle
		.path()
		.data_dir()
		.unwrap_or_else(|_| PathBuf::from("./"))
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
async fn refresh_menu_bar(node: tauri::State<'_, Arc<Node>>, app: AppHandle) -> Result<(), ()> {
	let has_library = !node.libraries.get_all().await.is_empty();
	menu::refresh_menu_bar(&app, has_library);
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

#[tauri::command(async)]
#[specta::specta]
async fn open_trash_in_os_explorer() -> Result<(), ()> {
	#[cfg(target_os = "macos")]
	{
		let full_path = format!("{}/.Trash/", std::env::var("HOME").unwrap());

		Command::new("open")
			.arg(full_path)
			.spawn()
			.map_err(|err| error!("Error opening trash: {err:#?}"))?
			.wait()
			.map_err(|err| error!("Error opening trash: {err:#?}"))?;

		Ok(())
	}

	#[cfg(target_os = "windows")]
	{
		Command::new("explorer")
			.arg("shell:RecycleBinFolder")
			.spawn()
			.map_err(|err| error!("Error opening trash: {err:#?}"))?
			.wait()
			.map_err(|err| error!("Error opening trash: {err:#?}"))?;
		return Ok(());
	}

	#[cfg(target_os = "linux")]
	{
		Command::new("xdg-open")
			.arg("trash://")
			.spawn()
			.map_err(|err| error!("Error opening trash: {err:#?}"))?
			.wait()
			.map_err(|err| error!("Error opening trash: {err:#?}"))?;

		Ok(())
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
pub enum DragAndDropEvent {
	Hovered { paths: Vec<String>, x: f64, y: f64 },
	Dropped { paths: Vec<String>, x: f64, y: f64 },
	Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DeepLinkEvent {
	data: String,
}

#[tokio::main]
async fn main() -> tauri::Result<()> {
	#[cfg(target_os = "linux")]
	sd_desktop_linux::normalize_environment();

	let builder = Builder::new()
		.commands(tauri_specta::collect_commands![
			app_ready,
			reset_spacedrive,
			open_logs_dir,
			refresh_menu_bar,
			reload_webview,
			set_menu_bar_item_state,
			request_fda_macos,
			open_trash_in_os_explorer,
			file::open_file_paths,
			file::open_ephemeral_files,
			file::get_file_path_open_with_apps,
			file::get_ephemeral_files_open_with_apps,
			file::open_file_path_with,
			file::open_ephemeral_file_with,
			file::reveal_items,
			theme::lock_app_theme,
			updater::check_for_update,
			updater::install_update
		])
		.events(collect_events![DragAndDropEvent]);

	#[cfg(debug_assertions)]
	builder
		.export(
			Typescript::default()
				.formatter(specta_typescript::formatter::prettier)
				.header("/* eslint-disable */"),
			"../src/commands.ts",
		)
		.expect("Failed to export typescript bindings");

	tauri::Builder::default()
		.invoke_handler(builder.invoke_handler())
		.plugin(tauri_plugin_deep_link::init())
		.setup(move |app| {
			// We need a the app handle to determine the data directory now.
			// This means all the setup code has to be within `setup`, however it doesn't support async so we `block_on`.
			let handle = app.handle().clone();
			app.listen("deep-link://new-url", move |event| {
				let deep_link_event = DeepLinkEvent {
					data: event.payload().to_string(),
				};
				println!("Deep link event={:?}", deep_link_event);

				handle.emit("deeplink", deep_link_event).unwrap();
			});

			block_in_place(|| {
				block_on(async move {
					builder.mount_events(app);

					let data_dir = app
						.path()
						.data_dir()
						.unwrap_or_else(|_| PathBuf::from("./"))
						.join("spacedrive");

					#[cfg(debug_assertions)]
					let data_dir = data_dir.join("dev");

					// The `_guard` must be assigned to variable for flushing remaining logs on main exit through Drop
					let (_guard, result) = match Node::init_logger(&data_dir) {
						Ok(guard) => (Some(guard), Node::new(data_dir).await),
						Err(err) => (None, Err(NodeError::Logger(err))),
					};

					let handle = app.handle();
					let (node, router) = match result {
						Ok(r) => r,
						Err(err) => {
							error!("Error starting up the node: {err:#?}");
							handle.plugin(sd_error_plugin(err))?;
							return Ok(());
						}
					};

					let should_clear_local_storage = node.libraries.get_all().await.is_empty();

					handle.plugin(rspc::integrations::tauri::plugin(router, {
						let node = node.clone();
						move || node.clone()
					}))?;
					handle.plugin(sd_server_plugin(node.clone()).await.unwrap())?; // TODO: Handle `unwrap`
					handle.manage(node.clone());

					handle.windows().iter().for_each(|(_, window)| {
						if should_clear_local_storage {
							debug!("cleaning localStorage");
							for webview in window.webviews() {
								webview.eval("localStorage.clear();").ok();
							}
						}

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
						window.set_decorations(false).unwrap();

						#[cfg(target_os = "macos")]
						{
							unsafe {
								sd_desktop_macos::set_titlebar_style(
									&window.ns_window().expect("NSWindows must exist on macOS"),
									false,
								);
								sd_desktop_macos::disable_app_nap(
									&"File indexer needs to run unimpeded".into(),
								);
							};
						}
					});

					Ok(())
				})
			})
		})
		.on_window_event(move |window, event| match event {
			// macOS expected behavior is for the app to not exit when the main window is closed.
			// Instead, the window is hidden and the dock icon remains so that on user click it should show the window again.
			#[cfg(target_os = "macos")]
			WindowEvent::CloseRequested { api, .. } => {
				// TODO: make this multi-window compatible in the future
				window
					.app_handle()
					.hide()
					.expect("Window should hide on macOS");
				api.prevent_close();
			}
			WindowEvent::Resized(_) => {
				let (_state, command) =
					if window.is_fullscreen().expect("Can't get fullscreen state") {
						(true, "window_fullscreened")
					} else {
						(false, "window_not_fullscreened")
					};

				window
					.emit("keybind", command)
					.expect("Unable to emit window event");

				#[cfg(target_os = "macos")]
				{
					let nswindow = window.ns_window().unwrap();
					unsafe { sd_desktop_macos::set_titlebar_style(&nswindow, _state) };
				}
			}
			_ => {}
		})
		.menu(menu::setup_menu)
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_os::init())
		.plugin(tauri_plugin_shell::init())
		.plugin(tauri_plugin_http::init())
		// TODO: Bring back Tauri Plugin Window State - it was buggy so we removed it.
		.plugin(tauri_plugin_updater::Builder::new().build())
		.plugin(updater::plugin())
		.manage(updater::State::default())
		.build(tauri::generate_context!())?
		.run(|_, _| {});

	Ok(())
}
