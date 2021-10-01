// use crate::filesystem::checksum;
use crate::filesystem::file;

use tauri::InvokeError;

#[tauri::command(async)]
pub async fn read_file_command(path: &str) -> Result<String, InvokeError> {
  let file = file::read_file(path)
    .await
    .map_err(|error| InvokeError::from(format!("Failed to read file: {}", error)))?;

  println!("file: {:?}", file);

  Ok("lol".to_owned())
}
// #[tauri::command(async)]
// pub async fn generate_buffer_checksum(path: &str) -> Result<File, InvokeError> {
//   let mut file = file::read_file(path)
//     .await
//     .map_err(|error| InvokeError::from(format!("Failed to read file: {}", error)))?;

//   // file.buffer_checksum = Some(checksum::create_hash(&file.uri).await.unwrap());
//   Ok(file)
// }
