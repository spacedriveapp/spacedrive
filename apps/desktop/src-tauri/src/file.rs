use std::sync::Arc;

use sd_core::Node;
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
