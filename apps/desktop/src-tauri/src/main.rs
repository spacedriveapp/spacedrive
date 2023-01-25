#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;

use sd_core::{Node, custom_uri::handle_custom_uri};
use tauri::async_runtime::block_on;
use tauri::{
	api::path,
	http::ResponseBuilder,
	Manager, RunEvent,
};
use http::Request;
use tokio::task::block_in_place;
use tokio::time::sleep;
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
async fn main() -> Result<(), Box<dyn Error>> {
	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	let (node, router) = Node::new(data_dir).await?;

	let app = tauri::Builder::default()
		.plugin(rspc::integrations::tauri::plugin(router, {
			let node = node.clone();
			move || node.get_request_context()
		}))
		.register_uri_scheme_protocol("spacedrive", {
			let node = node.clone();
			move |_, req| {
				let uri = req.uri();
				let uri = uri.replace("spacedrive://", "https://spacedrive.localhost/"); // Convert Mac and Linux style URI to Windows style URI. Windows style is valid so it can be put into a `http::Request`.

				// Encoded by `convertFileSrc` on the frontend
				let uri = percent_encoding::percent_decode(uri.as_bytes())
					.decode_utf8_lossy()
					.to_string();

				let mut r = Request::builder()
					.method(req.method())
					.uri(uri);
				for (key, value) in req.headers() {
					r = r.header(key, value);
				}
				let r = r.body(req.body().clone()).unwrap(); // TODO: This clone feels so unnecessary but Tauri pass `req` as a reference so we can get the owned value.

				// TODO: This blocking sucks but is required for now. https://github.com/tauri-apps/wry/issues/420
				let resp =
					block_in_place(|| block_on(handle_custom_uri(&node, r))).unwrap_or_else(|err| err.into_response().unwrap());
				let mut r = ResponseBuilder::new()
					.version(resp.version())
					.status(resp.status());

				for (key, value) in resp.headers() {
					r = r.header(key, value);
				}

				r.body(resp.into_body())
			}
		})
		.setup(|app| {
			let app = app.handle();
			app.windows().iter().for_each(|(_, window)| {
				// window.hide().unwrap();

				tokio::spawn({
					let window = window.clone();
					async move {
						sleep(Duration::from_secs(3)).await;
						if !window.is_visible().unwrap_or(true) {
							println!("Window did not emit `app_ready` event fast enough. Showing window...");
							let _ = window.show();
						}
					}
				});

				#[cfg(target_os = "windows")]
				window.set_decorations(true).unwrap();

				#[cfg(target_os = "macos")]
				{
					use macos::*;

					let window = window.ns_window().unwrap();

					unsafe { set_titlebar_style(&window, true, true) };
					unsafe { blur_window_background(&window) };
				}
			});

			Ok(())
		})
		.on_menu_event(menu::handle_menu_event)
		.invoke_handler(tauri::generate_handler![app_ready])
		.menu(menu::get_menu())
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

			block_in_place(|| block_on(node.shutdown()));
			app_handler.exit(0);
		}
	});

	Ok(())
}
