use crate::filesystem::checksum;
use crate::filesystem::file;
use crate::filesystem::file::File;

use tauri::InvokeError;

#[tauri::command(async)]
pub async fn read_file_command(path: &str) -> Result<File, InvokeError> {
  let file = file::read_file(path)
    .await
    .map_err(|error| InvokeError::from(format!("Failed to read file: {}", error)))?;

  // file::commit_file(&file).await;

  Ok(file)
}
#[tauri::command(async)]
pub async fn generate_buffer_checksum(path: &str) -> Result<File, InvokeError> {
  let mut file = file::read_file(path)
    .await
    .map_err(|error| InvokeError::from(format!("Failed to read file: {}", error)))?;

  // file.buffer_checksum = Some(checksum::create_hash(&file.uri).await.unwrap());
  Ok(file)
}
