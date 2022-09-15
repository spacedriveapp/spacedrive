use std::env::consts;

use tauri::{
	AboutMetadata, CustomMenuItem, Manager, Menu, MenuItem, Submenu, WindowMenuEvent, Wry,
};

pub(crate) fn get_menu() -> Menu {
	match consts::OS {
		"macos" => custom_menu_bar(),
		_ => Menu::new(),
	}
}

fn custom_menu_bar() -> Menu {
	let app_menu = Menu::new()
		.add_native_item(MenuItem::About(
			"Spacedrive".to_string(),
			AboutMetadata::new(),
		)) // TODO: fill out about metadata
		.add_native_item(MenuItem::Separator)
		.add_item(
			CustomMenuItem::new("open_settings".to_string(), "Settings...")
				.accelerator("CmdOrCtrl+Comma"),
		)
		.add_native_item(MenuItem::Separator)
		.add_native_item(MenuItem::Services)
		.add_native_item(MenuItem::Separator)
		.add_native_item(MenuItem::Hide)
		.add_native_item(MenuItem::HideOthers)
		.add_native_item(MenuItem::ShowAll)
		.add_native_item(MenuItem::Separator)
		.add_native_item(MenuItem::Quit);

	let file_menu = Menu::new()
		.add_item(
			CustomMenuItem::new("new_window".to_string(), "New Window")
				.accelerator("CmdOrCtrl+N")
				.disabled(),
		)
		.add_item(
			CustomMenuItem::new("close".to_string(), "Close Window").accelerator("CmdOrCtrl+W"),
		);
	let edit_menu = Menu::new()
		.add_native_item(MenuItem::Copy)
		.add_native_item(MenuItem::Paste)
		.add_native_item(MenuItem::SelectAll);
	let view_menu = Menu::new()
		.add_item(CustomMenuItem::new("open_search".to_string(), "Search...").accelerator("CmdOrCtrl+F"))
		// .add_item(
		// 	CustomMenuItem::new("command_pallete".to_string(), "Command Pallete")
		// 		.accelerator("CmdOrCtrl+P"),
		// )
		.add_item(CustomMenuItem::new("layout".to_string(), "Layout").disabled());
	let window_menu = Menu::new().add_native_item(MenuItem::EnterFullScreen);

	#[cfg(debug_assertions)]
	let view_menu = {
		let view_menu = view_menu.add_native_item(MenuItem::Separator);

		#[cfg(target_os = "macos")]
		let view_menu = view_menu.add_item(
			CustomMenuItem::new("reload_app".to_string(), "Reload").accelerator("CmdOrCtrl+R"),
		);

		view_menu.add_item(
			CustomMenuItem::new("toggle_devtools".to_string(), "Toggle Developer Tools")
				.accelerator("CmdOrCtrl+Alt+I"),
		)
	};

	Menu::new()
		.add_submenu(Submenu::new("Spacedrive", app_menu))
		.add_submenu(Submenu::new("File", file_menu))
		.add_submenu(Submenu::new("Edit", edit_menu))
		.add_submenu(Submenu::new("View", view_menu))
		.add_submenu(Submenu::new("Window", window_menu))
}

pub(crate) fn handle_menu_event(event: WindowMenuEvent<Wry>) {
	match event.menu_item_id() {
		"quit" => {
			let app = event.window().app_handle();
			app.exit(0);
		}
		"open_settings" => event.window().emit("keybind", "open_settings").unwrap(),
		"close" => {
			let window = event.window();

			#[cfg(debug_assertions)]
			if window.is_devtools_open() {
				window.close_devtools();
			} else {
				window.close().unwrap();
			}

			#[cfg(not(debug_assertions))]
			window.close().unwrap();
		}
		"open_search" => event
			.window()
			.emit("keybind", "open_search".to_string())
			.unwrap(),
		"reload_app" => {
			#[cfg(target_os = "macos")]
			{
				event
					.window()
					.with_webview(|webview| {
						use crate::macos::reload_webview;

						reload_webview(webview.inner() as _);
					})
					.unwrap();
			}

			#[cfg(not(target_os = "macos"))]
			{
				unimplemented!();
			}
		}
		#[cfg(debug_assertions)]
		"toggle_devtools" => {
			let window = event.window();

			if window.is_devtools_open() {
				window.close_devtools();
			} else {
				window.open_devtools();
			}
		}
		_ => {}
	}
}
