#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use sd_core::{Node, NodeError};

use tauri::{api::path, ipc::RemoteDomainAccessScope, AppHandle, Manager};
use tauri_plugins::{sd_error_plugin, sd_server_plugin};
use tokio::time::sleep;
use tracing::error;

mod tauri_plugins;

mod theme;

mod file;
mod menu;

#[tauri::command(async)]
#[specta::specta]
async fn app_ready(app_handle: AppHandle) {
	let window = app_handle.get_window("main").unwrap();

	window.show().unwrap();
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
async fn open_logs_dir(node: tauri::State<'_, Arc<Node>>) -> Result<(), ()> {
	let logs_path = node.data_dir.join("logs");

	#[cfg(target_os = "linux")]
	let open_result = sd_desktop_linux::open_file_path(&logs_path);

	#[cfg(not(target_os = "linux"))]
	let open_result = opener::open(logs_path);

	open_result.map_err(|err| {
		error!("Failed to open logs dir: {err}");
	})
}

// TODO(@Oscar): A helper like this should probs exist in tauri-specta
macro_rules! tauri_handlers {
	($($name:path),+) => {{
		#[cfg(debug_assertions)]
		tauri_specta::ts::export(specta::collect_types![$($name),+], "../src/commands.ts").unwrap();

		tauri::generate_handler![$($name),+]
	}};
}

#[tokio::main]
async fn main() -> tauri::Result<()> {
	dotenv::dotenv().ok();

	#[cfg(target_os = "linux")]
	sd_desktop_linux::normalize_environment();

	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	#[cfg(debug_assertions)]
	let data_dir = data_dir.join("dev");

	// The `_guard` must be assigned to variable for flushing remaining logs on main exit through Drop
	let (_guard, result) = match Node::init_logger(&data_dir) {
		Ok(guard) => (Some(guard), Node::new(data_dir).await),
		Err(err) => (None, Err(NodeError::Logger(err))),
	};

	let app = tauri::Builder::default();
	let app = match result {
		Ok((node, router)) => app
			.plugin(rspc::integrations::tauri::plugin(router, {
				let node = node.clone();
				move || node.clone()
			}))
			.plugin(sd_server_plugin(node.clone()).unwrap()) // TODO: Handle `unwrap`
			.manage(node),
		Err(err) => {
			error!("Error starting up the node: {err}");
			app.plugin(sd_error_plugin(err))
		}
	};

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

	let app = app
		.setup(|app| {
			#[cfg(feature = "updater")]
			tauri::updater::builder(app.handle()).should_install(|_current, _latest| true);

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
					use sd_desktop_macos::*;

					let window = window.ns_window().unwrap();

					unsafe { set_titlebar_style(&window, true, true) };
					unsafe { blur_window_background(&window) };
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
		.menu(menu::get_menu())
		.invoke_handler(tauri_handlers![
			app_ready,
			reset_spacedrive,
			open_logs_dir,
			file::open_file_paths,
			file::get_file_path_open_with_apps,
			file::open_file_path_with,
			file::reveal_items,
			theme::lock_app_theme
		])
		.build(tauri::generate_context!())?;

	app.run(|_, _| {});
	Ok(())
}
