use std::sync::Arc;

use sd_core::Node;
use sd_desktop_macos::OpenWithApplication;
use serde::Serialize;
use specta::Type;

#[derive(Serialize, Type)]
#[serde(tag = "t", content = "c")]
pub enum OpenFilePathResult {
	NoLibrary,
	NoFile,
	OpenError(String),
	AllGood,
}

#[tauri::command(async)]
#[specta::specta]
pub async fn open_file_path(
	library: uuid::Uuid,
	id: i32,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<OpenFilePathResult, ()> {
	let res = if let Some(library) = node.library_manager.get_library(library).await {
		let Ok(Some(path)) = library
			.get_file_path(id)
			.await
            else {
                return Ok(OpenFilePathResult::NoFile)
            };

		opener::open(path)
			.map(|_| OpenFilePathResult::AllGood)
			.unwrap_or_else(|e| OpenFilePathResult::OpenError(e.to_string()))
	} else {
		OpenFilePathResult::NoLibrary
	};

	Ok(res)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn bruh(
	library: uuid::Uuid,
	id: i32,
	node: tauri::State<'_, Arc<Node>>,
) -> Result<(), ()> {
	if let Some(library) = node.library_manager.get_library(library).await {
		let Ok(Some(path)) = library
			.get_file_path(id)
			.await
            else {
                return Err(())
            };

		unsafe { sd_desktop_macos::get_open_with_applications(&path.to_str().unwrap().into()) }
			.as_slice()
			.iter()
			.map(|a| OpenWithApplication {
				name: a.name.to_string(),
			})
			.collect::<Vec<_>>();

		return Ok(());
	};

	Err(())
}
