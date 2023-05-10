#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

use std::{path::PathBuf, time::Duration};

use sd_core::{custom_uri::create_custom_uri_endpoint, Node, NodeError};

use tauri::{api::path, async_runtime::block_on, plugin::TauriPlugin, Manager, RunEvent, Runtime};
use tokio::{task::block_in_place, time::sleep};
use tracing::{debug, error};

#[cfg(target_os = "linux")]
mod app_linux;

mod file;
mod menu;

#[tauri::command(async)]
#[specta::specta]
async fn app_ready(app_handle: tauri::AppHandle) {
	let window = app_handle.get_window("main").unwrap();

	window.show().unwrap();
}

pub fn tauri_error_plugin<R: Runtime>(err: NodeError) -> TauriPlugin<R> {
	tauri::plugin::Builder::new("spacedrive")
		.js_init_script(format!(r#"window.__SD_ERROR__ = "{err}";"#))
		.build()
}

macro_rules! tauri_handlers {
	($($name:path),+) => {{
		#[cfg(debug_assertions)]
		tauri_specta::ts::export(specta::collect_types![$($name),+], "../src/commands.ts").unwrap();

		tauri::generate_handler![$($name),+]
	}};
}

#[tokio::main]
async fn main() -> tauri::Result<()> {
	#[cfg(target_os = "linux")]
	let (tx, rx) = tokio::sync::mpsc::channel(1);

	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	#[cfg(debug_assertions)]
	let data_dir = data_dir.join("dev");

	let result = Node::new(data_dir).await;

	let app = tauri::Builder::default();
	let (node, app) = match result {
		Ok((node, router)) => {
			// This is a super cringe workaround for: https://github.com/tauri-apps/tauri/issues/3725 & https://bugs.webkit.org/show_bug.cgi?id=146351#c5
			let endpoint = create_custom_uri_endpoint(node.clone());

			#[cfg(target_os = "linux")]
			let app = app_linux::setup(app, rx, endpoint).await;

			#[cfg(not(target_os = "linux"))]
			let app = app.register_uri_scheme_protocol(
				"spacedrive",
				endpoint.tauri_uri_scheme("spacedrive"),
			);

			let app = app
				.plugin(rspc::integrations::tauri::plugin(router, {
					let node = node.clone();
					move || node.clone()
				}))
				.manage(node.clone());

			(Some(node), app)
		}
		Err(err) => (None, app.plugin(tauri_error_plugin(err))),
	};

	let app = app
		.setup(|app| {
			#[cfg(feature = "updater")]
			tauri::updater::builder(app.handle()).should_install(|_current, _latest| true);

			let app = app.handle();

			app.windows().iter().for_each(|(_, window)| {
				// window.hide().unwrap();

				tokio::spawn({
					let window = window.clone();
					async move {
						sleep(Duration::from_secs(3)).await;
						if !window.is_visible().unwrap_or(true) {
							println!(
							"Window did not emit `app_ready` event fast enough. Showing window..."
						);
							let _ = window.show();
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

			Ok(())
		})
		.on_menu_event(menu::handle_menu_event)
		.menu(menu::get_menu())
		.invoke_handler(tauri_handlers![
			app_ready,
			file::open_file_path,
			file::get_file_path_open_with_apps,
			file::open_file_path_with
		])
		.build(tauri::generate_context!())?;

	app.run(move |app_handler, event| {
		if let RunEvent::ExitRequested { .. } = event {
			debug!("Closing all open windows...");
			app_handler
				.windows()
				.iter()
				.for_each(|(window_name, window)| {
					debug!("closing window: {window_name}");
					if let Err(e) = window.close() {
						error!("failed to close window '{}': {:#?}", window_name, e);
					}
				});

			if let Some(node) = &node {
				block_in_place(|| block_on(node.shutdown()));
			}

			#[cfg(target_os = "linux")]
			block_in_place(|| block_on(tx.send(()))).ok();

			app_handler.exit(0);
		}
	});

	Ok(())
}
