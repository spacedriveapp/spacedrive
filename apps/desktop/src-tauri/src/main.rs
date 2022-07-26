use sdcore::Node;
use tauri::{api::path, Manager};
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
	let mut data_dir = path::data_dir().unwrap_or(std::path::PathBuf::from("./"));
	data_dir = data_dir.join("spacedrive");

	let (node, router) = Node::new(data_dir).await;
	tauri::Builder::default()
		.plugin(sdcore::rspc::integrations::tauri::plugin(
			router,
			move || node.get_request_context(),
		))
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
		.on_menu_event(|event| menu::handle_menu_event(event))
		.invoke_handler(tauri::generate_handler![app_ready,])
		.menu(menu::get_menu())
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
