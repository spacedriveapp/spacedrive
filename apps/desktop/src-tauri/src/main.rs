use std::time::{Duration, Instant};

use sdcore::{ClientCommand, ClientQuery, CoreController, CoreEvent, CoreResponse, Node};
use tauri::api::path;
use tauri::Manager;

#[cfg(target_os = "macos")]
mod macos;
mod menu;

#[tauri::command(async)]
async fn client_query_transport(
	core: tauri::State<'_, CoreController>,
	data: ClientQuery,
) -> Result<CoreResponse, String> {
	match core.query(data).await {
		Ok(response) => Ok(response),
		Err(err) => {
			println!("query error: {:?}", err);
			Err(err.to_string())
		}
	}
}

#[tauri::command(async)]
async fn client_command_transport(
	core: tauri::State<'_, CoreController>,
	data: ClientCommand,
) -> Result<CoreResponse, String> {
	match core.command(data).await {
		Ok(response) => Ok(response),
		Err(err) => {
			println!("command error: {:?}", err);
			Err(err.to_string())
		}
	}
}

#[tauri::command(async)]
async fn app_ready(app_handle: tauri::AppHandle) {
	let window = app_handle.get_window("main").unwrap();

	window.show().unwrap();
}

#[tokio::main]
async fn main() {
	let data_dir = path::data_dir().unwrap_or(std::path::PathBuf::from("./"));
	// create an instance of the core
	let (mut node, mut event_receiver) = Node::new(data_dir).await;
	// run startup tasks
	node.initializer().await;
	// extract the node controller
	let controller = node.get_controller();
	// throw the node into a dedicated thread
	tokio::spawn(async move {
		node.start().await;
	});
	// create tauri app
	tauri::Builder::default()
		// pass controller to the tauri state manager
		.manage(controller)
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

			// core event transport
			tokio::spawn(async move {
				let mut last = Instant::now();
				// handle stream output
				while let Some(event) = event_receiver.recv().await {
					match event {
						CoreEvent::InvalidateQueryDebounced(_) => {
							let current = Instant::now();
							if current.duration_since(last) > Duration::from_millis(1000 / 60) {
								last = current;
								app.emit_all("core_event", &event).unwrap();
							}
						}
						event => {
							app.emit_all("core_event", &event).unwrap();
						}
					}
				}
			});

			Ok(())
		})
		.on_menu_event(|event| menu::handle_menu_event(event))
		.invoke_handler(tauri::generate_handler![
			client_query_transport,
			client_command_transport,
			app_ready,
		])
		.menu(menu::get_menu())
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
