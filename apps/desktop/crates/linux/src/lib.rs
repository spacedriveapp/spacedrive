#![cfg(target_os = "linux")]

mod app_info;
mod env;

pub use app_info::{list_apps_associated_with_ext, open_files_path_with};
pub use env::normalize_environment;
