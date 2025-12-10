use std::path::{Path, PathBuf};
use tracing::error;

/// Reveal a file in the native file manager (Finder on macOS, Explorer on Windows, etc.)
#[tauri::command]
pub async fn reveal_file(path: String) -> Result<(), String> {
	let path = PathBuf::from(path);

	if !path.exists() {
		return Err(format!("Path does not exist: {}", path.display()));
	}

	reveal_path(&path).map_err(|e| {
		error!("Failed to reveal file: {:#?}", e);
		format!("Failed to reveal file: {}", e)
	})
}

/// Get the physical path to a sidecar file
#[tauri::command]
pub async fn get_sidecar_path(
	library_id: String,
	content_uuid: String,
	kind: String,
	variant: String,
	format: String,
) -> Result<String, String> {
	// Get the data directory
	let data_dir = sd_core::config::default_data_dir()
		.map_err(|e| format!("Failed to get data directory: {}", e))?;

	// Find the actual library folder (might be named differently than the ID)
	let library_folder = find_library_folder(&data_dir, &library_id)
		.await
		.map_err(|e| format!("Failed to find library folder: {:?}", e))?;

	// Actual path structure: sidecars/content/{first2}/{next2}/{uuid}/{kind}s/{variant}.{format}
	// Example: sidecars/content/35/3c/353c7043-8d28-56ec-a424-7ab8932b1ffe/thumbs/detail@1x.webp
	let first_two = &content_uuid[0..2];
	let next_two = &content_uuid[2..4];

	// Special case: "transcript" stays singular (not "transcripts")
	let kind_dir = if kind == "transcript" {
		kind
	} else {
		format!("{}s", kind) // "thumb" -> "thumbs"
	};

	let sidecar_path = library_folder
		.join("sidecars")
		.join("content")
		.join(first_two)
		.join(next_two)
		.join(&content_uuid)
		.join(&kind_dir)
		.join(format!("{}.{}", variant, format));

	Ok(sidecar_path.to_string_lossy().to_string())
}

/// Find library folder by UUID (reads library.json files to match ID)
async fn find_library_folder(data_dir: &Path, library_id: &str) -> Result<PathBuf, String> {
	let libraries_dir = data_dir.join("libraries");

	// Read all .sdlibrary folders
	let mut entries = tokio::fs::read_dir(&libraries_dir)
		.await
		.map_err(|e| format!("Failed to read libraries directory: {}", e))?;

	while let Some(entry) = entries
		.next_entry()
		.await
		.map_err(|e| format!("Failed to read directory entry: {}", e))?
	{
		let path = entry.path();
		if path.extension().and_then(|s| s.to_str()) == Some("sdlibrary") {
			// Try to read library.json
			let library_json_path = path.join("library.json");
			if let Ok(contents) = tokio::fs::read_to_string(&library_json_path).await {
				if let Ok(json) = serde_json::from_str::<serde_json::Value>(&contents) {
					if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
						if id == library_id {
							return Ok(path);
						}
					}
				}
			}
		}
	}

	Err(format!("Library folder not found for ID: {}", library_id))
}

#[cfg(target_os = "macos")]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	std::process::Command::new("open")
		.arg("-R")
		.arg(path)
		.spawn()?
		.wait()?;
	Ok(())
}

#[cfg(target_os = "windows")]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	std::process::Command::new("explorer")
		.arg("/select,")
		.arg(path)
		.spawn()?
		.wait()?;
	Ok(())
}

#[cfg(target_os = "linux")]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	// On Linux, we'll try to open the parent directory
	// Different desktop environments have different file managers
	if let Some(parent) = path.parent() {
		std::process::Command::new("xdg-open")
			.arg(parent)
			.spawn()?
			.wait()?;
	}
	Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn reveal_path(path: &Path) -> Result<(), std::io::Error> {
	Err(std::io::Error::new(
		std::io::ErrorKind::Unsupported,
		"Reveal is not supported on this platform",
	))
}
