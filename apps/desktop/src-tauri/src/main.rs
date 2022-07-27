use std::path::PathBuf;
use std::time::{Duration, Instant};

use dotenvy::dotenv;
use futures::executor::block_on;
use log::{debug, error, info};
use sdcore::{ClientCommand, ClientQuery, CoreEvent, CoreResponse, Node, NodeController};
use tauri::{api::path, Manager, RunEvent};
use tokio::sync::oneshot;

#[cfg(target_os = "macos")]
mod macos;
mod menu;

#[tauri::command(async)]
async fn client_query_transport(
	core: tauri::State<'_, NodeController>,
	data: ClientQuery,
) -> Result<CoreResponse, String> {
	match core.query(data).await {
		Ok(response) => Ok(response),
		Err(err) => {
			error!("query error: {:?}", err);
			Err(err.to_string())
		}
	}
}

#[tauri::command(async)]
async fn client_command_transport(
	core: tauri::State<'_, NodeController>,
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

struct ShutdownManager {
	shutdown_tx: Option<oneshot::Sender<()>>,
	shutdown_completion_rx: Option<oneshot::Receiver<()>>,
}

impl ShutdownManager {
	fn shutdown(&mut self) {
		if let Some(sender) = self.shutdown_tx.take() {
			sender.send(()).unwrap();
			if let Some(receiver) = self.shutdown_completion_rx.take() {
				block_on(receiver).expect("failed to receive shutdown completion signal");
			}
		}
	}
}

#[tokio::main]
async fn main() {
	dotenv().ok();
	env_logger::init();

	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	// create an instance of the core
	let (controller, mut event_receiver, node, shutdown_completion_rx) = Node::new(data_dir).await;
	let (shutdown_tx, shutdown_rx) = oneshot::channel();
	let mut shutdown_manager = ShutdownManager {
		shutdown_tx: Some(shutdown_tx),
		shutdown_completion_rx: Some(shutdown_completion_rx),
	};

	tokio::spawn(node.start(shutdown_rx));
	// create tauri app
	let app = tauri::Builder::default()
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
		.on_menu_event(menu::handle_menu_event)
		.invoke_handler(tauri::generate_handler![
			client_query_transport,
			client_command_transport,
			app_ready,
		])
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
			info!("Spacedrive shutting down...");
			shutdown_manager.shutdown();
			app_handler.exit(0);
		}
	})
}
