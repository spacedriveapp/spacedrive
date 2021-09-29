use crate::filesystem::file;
use crate::filesystem::file::File;
use tauri::InvokeError;

#[tauri::command(async)]
pub async fn read_file_command(path: &str) -> Result<File, InvokeError> {
  let file = file::read_file(path)
    .await
    .map_err(|error| InvokeError::from(format!("Failed to read file: {}", error)))?;
  Ok(file)
}
