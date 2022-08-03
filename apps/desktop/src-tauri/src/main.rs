use std::path::PathBuf;

use sdcore::Node;
use tauri::{api::path, Manager, RunEvent};
use tracing::{debug, error};
#[cfg(target_os = "macos")]
mod macos;
mod menu;

#[tauri::command(async)]
async fn app_ready(app_handle: tauri::AppHandle) {
	let window = app_handle.get_window("main").unwrap();

	window.show().unwrap();
}

#[tokio::main]
async fn main() {
	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	let (node, router) = Node::new(data_dir).await;

	let app = tauri::Builder::default()
		.plugin(sdcore::rspc::integrations::tauri::plugin(router, {
			let node = node.clone();
			move || node.get_request_context()
		}))
		.setup(|app| {
			let app = app.handle();

			#[cfg(target_os = "macos")]
			{
				use macos::{lock_app_theme, AppThemeType};

				lock_app_theme(AppThemeType::Dark as _);
			}

			app.windows().iter().for_each(|(_, window)| {
				window.hide().unwrap();

				#[cfg(target_os = "windows")]
				window.set_decorations(true).unwrap();

				#[cfg(target_os = "macos")]
				{
					use macos::*;

					let window = window.ns_window().unwrap();
					set_titlebar_style(window, true, true);
					blur_window_background(window);
				}
			});

			Ok(())
		})
		.on_menu_event(menu::handle_menu_event)
		.invoke_handler(tauri::generate_handler![app_ready,])
		.menu(menu::get_menu())
		.build(tauri::generate_context!())
		.expect("error while building tauri application");

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

			node.shutdown();
			app_handler.exit(0);
		}
	})
}
