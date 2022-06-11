use std::env::consts;

use tauri::{AboutMetadata, CustomMenuItem, Menu, MenuItem, Submenu, WindowMenuEvent, Wry};

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
		.add_native_item(MenuItem::Paste);
	let view_menu = Menu::new()
		.add_item(
			CustomMenuItem::new("command_pallete".to_string(), "Command Pallete")
				.accelerator("CmdOrCtrl+P"),
		)
		.add_item(CustomMenuItem::new("layout".to_string(), "Layout").disabled());
	let window_menu = Menu::new().add_native_item(MenuItem::EnterFullScreen);

	let window = Submenu::new(
		"Window",
		Menu::new().add_native_item(MenuItem::EnterFullScreen),
	);

	let menu = Menu::new()
		.add_submenu(Submenu::new("Spacedrive", app_menu))
		.add_submenu(Submenu::new("File", file_menu))
		.add_submenu(Submenu::new("Edit", edit_menu))
		.add_submenu(Submenu::new("View", view_menu))
		.add_submenu(Submenu::new("Window", window_menu));

	menu
}

pub(crate) fn handle_menu_event(event: WindowMenuEvent<Wry>) {
	match event.menu_item_id() {
		"quit" => {
			std::process::exit(0);
		}
		"close" => {
			event.window().close().unwrap();
		}
		_ => {}
	}
}
