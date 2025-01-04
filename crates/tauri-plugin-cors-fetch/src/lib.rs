// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! ![tauri-plugin-cors-fetch](https://github.com/idootop/tauri-plugin-cors-fetch/raw/main/banner.png)
//!
//! Enabling Cross-Origin Resource Sharing (CORS) for Fetch Requests within Tauri applications.

use std::path::PathBuf;

use once_cell::sync::OnceCell;
pub use reqwest;
use tauri::{
	plugin::{Builder, TauriPlugin},
	Manager, Runtime,
};

pub use error::{Error, Result};
mod commands;
mod error;

pub static NODE_DATA_DIR: OnceCell<PathBuf> = OnceCell::new();

pub fn init<R: Runtime>() -> TauriPlugin<R> {
	Builder::<R>::new("cors-fetch")
		.invoke_handler(tauri::generate_handler![
			commands::cors_request,
			commands::cancel_cors_request,
		])
		.setup(|app_handle, _| {
			let data_dir = app_handle
				.path()
				.data_dir()
				.unwrap_or_else(|_| PathBuf::from("./"));

			NODE_DATA_DIR.set(data_dir).unwrap();

			Ok(())
		})
		.build()
}
