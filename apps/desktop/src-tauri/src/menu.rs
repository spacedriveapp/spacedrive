use tauri::{Manager, Menu, WindowMenuEvent, Wry};

#[cfg(target_os = "macos")]
use tauri::{AboutMetadata, CustomMenuItem, MenuItem, Submenu};

pub fn get_menu() -> Menu {
	#[cfg(target_os = "macos")]
	{
		custom_menu_bar()
	}
	#[cfg(not(target_os = "macos"))]
	{
		Menu::new()
	}
}

// update this whenever you add something which requires a valid library to use
#[cfg(target_os = "macos")]
const LIBRARY_LOCKED_MENU_IDS: [&str; 12] = [
	"new_window",
	"open_overview",
	"open_search",
	"open_settings",
	"reload_explorer",
	"layout_grid",
	"layout_list",
	"layout_media",
	"new_file",
	"new_directory",
	"new_library", // disabled because the first one should at least be done via onboarding
	"add_location",
];

#[cfg(target_os = "macos")]
fn custom_menu_bar() -> Menu {
	let app_menu = Menu::new()
		.add_native_item(MenuItem::About(
			"Spacedrive".to_string(),
			AboutMetadata::new()
				.authors(vec!["Spacedrive Technology Inc.".to_string()])
				.license("AGPL-3.0-only")
				.version(env!("CARGO_PKG_VERSION"))
				.website("https://spacedrive.com/")
				.website_label("Spacedrive.com"),
		))
		.add_native_item(MenuItem::Separator)
		.add_item(CustomMenuItem::new("new_library", "New Library").disabled()) // TODO(brxken128): add keybind handling here
		.add_submenu(Submenu::new(
			"Library",
			Menu::new()
				.add_item(CustomMenuItem::new("library_<uuid>", "Library 1").disabled())
				.add_item(CustomMenuItem::new("library_<uuid2>", "Library 2").disabled()), // TODO: enumerate libraries and make this a library selector
		))
		.add_native_item(MenuItem::Separator)
		.add_native_item(MenuItem::Hide)
		.add_native_item(MenuItem::HideOthers)
		.add_native_item(MenuItem::ShowAll)
		.add_native_item(MenuItem::Separator)
		.add_native_item(MenuItem::Quit);

	let file_menu = Menu::new()
		.add_item(
			CustomMenuItem::new("new_file", "New File")
				.accelerator("CmdOrCtrl+N")
				.disabled(), // TODO(brxken128): add keybind handling here
		)
		.add_item(
			CustomMenuItem::new("new_directory", "New Directory")
				.accelerator("CmdOrCtrl+D")
				.disabled(), // TODO(brxken128): add keybind handling here
		)
		.add_item(CustomMenuItem::new("add_location", "Add Location").disabled()); // TODO(brxken128): add keybind handling here;

	let edit_menu = Menu::new()
		.add_native_item(MenuItem::Separator)
		.add_native_item(MenuItem::Copy)
		.add_native_item(MenuItem::Cut)
		.add_native_item(MenuItem::Paste)
		.add_native_item(MenuItem::Redo)
		.add_native_item(MenuItem::Undo)
		.add_native_item(MenuItem::SelectAll);

	let view_menu = Menu::new()
		.add_item(CustomMenuItem::new("open_overview", "Overview").accelerator("CmdOrCtrl+."))
		.add_item(CustomMenuItem::new("open_search", "Search").accelerator("CmdOrCtrl+F"))
		.add_item(CustomMenuItem::new("open_settings", "Settings").accelerator("CmdOrCtrl+Comma"))
		.add_item(
			CustomMenuItem::new("reload_explorer", "Reload explorer")
				.accelerator("CmdOrCtrl+R")
				.disabled(),
		)
		.add_submenu(Submenu::new(
			"Layout",
			Menu::new()
				.add_item(CustomMenuItem::new("layout_grid", "Grid (Default)").disabled())
				.add_item(CustomMenuItem::new("layout_list", "List").disabled())
				.add_item(CustomMenuItem::new("layout_media", "Media").disabled()),
		));
	// .add_item(
	// 	CustomMenuItem::new("command_pallete", "Command Pallete")
	// 		.accelerator("CmdOrCtrl+P"),
	// )

	#[cfg(debug_assertions)]
	let view_menu = view_menu.add_native_item(MenuItem::Separator).add_item(
		CustomMenuItem::new("toggle_devtools", "Toggle Developer Tools")
			.accelerator("CmdOrCtrl+Shift+Alt+I"),
	);

	let window_menu = Menu::new()
		.add_native_item(MenuItem::Minimize)
		.add_native_item(MenuItem::Zoom)
		.add_item(
			CustomMenuItem::new("new_window", "New Window")
				.accelerator("CmdOrCtrl+Shift+N")
				.disabled(),
		)
		.add_item(CustomMenuItem::new("close_window", "Close Window").accelerator("CmdOrCtrl+W"))
		.add_native_item(MenuItem::EnterFullScreen)
		.add_native_item(MenuItem::Separator)
		.add_item(
			CustomMenuItem::new("reload_app", "Reload Webview")
				.accelerator("CmdOrCtrl+Shift+Alt+R"),
		);

	Menu::new()
		.add_submenu(Submenu::new("Spacedrive", app_menu))
		.add_submenu(Submenu::new("File", file_menu))
		.add_submenu(Submenu::new("Edit", edit_menu))
		.add_submenu(Submenu::new("View", view_menu))
		.add_submenu(Submenu::new("Window", window_menu))
}

pub fn handle_menu_event(event: WindowMenuEvent<Wry>) {
	match event.menu_item_id() {
		"quit" => {
			let app = event.window().app_handle();
			app.exit(0);
		}
		"reload_explorer" => event.window().emit("keybind", "reload_explorer").unwrap(),
		"open_settings" => event.window().emit("keybind", "open_settings").unwrap(),
		"open_overview" => event.window().emit("keybind", "open_overview").unwrap(),
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
			event
				.window()
				.with_webview(crate::reload_webview_inner)
				.expect("Error while reloading webview");
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

/// If any are explicitly marked with `.disabled()` in the `custom_menu_bar()` function, this won't have an effect.
/// We include them in the locked menu IDs anyway for future-proofing, in-case someone forgets.
#[cfg(target_os = "macos")]
pub fn set_library_locked_menu_items_enabled(
	handle: tauri::window::MenuHandle,
	enabled: bool,
) {
	LIBRARY_LOCKED_MENU_IDS
		.iter()
		.try_for_each(|id| handle.get_item(id).set_enabled(enabled))
		.expect("Unable to disable menu items (there are no libraries present, so certain options should be hidden)");
}
