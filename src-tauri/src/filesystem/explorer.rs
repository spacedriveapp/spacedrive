use crate::db::entity;
use crate::filesystem;
use anyhow::Result;
use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
  entry
    .file_name()
    .to_str()
    .map(|s| s.starts_with("."))
    .unwrap_or(false)
}
fn is_app_bundle(entry: &DirEntry) -> bool {
  let is_dir = entry.metadata().unwrap().is_dir();
  let contains_dot = entry
    .file_name()
    .to_str()
    .map(|s| s.contains("."))
    .unwrap_or(false);

  is_dir && contains_dot
}

pub async fn scan(path: &str) -> Result<()> {
  println!("Scanning directory: {}", &path);
  // read the scan directory
  let file_or_dir = filesystem::reader::path(path, None).await?;

  if let Some(dir) = file_or_dir.dir {
    let mut current_dir: entity::dir::Model = dir;
    for entry in WalkDir::new(path)
      .into_iter()
      .filter_entry(|e| !is_hidden(e) && !is_app_bundle(e))
    {
      let entry = entry?;
      let path = entry.path().to_str().unwrap();

      let child_file_or_dir = filesystem::reader::path(&path, Some(current_dir.id))
        .await
        .unwrap_or_else(|e| {
          println!("could not read path, {}", e);
          return filesystem::reader::FileOrDir {
            dir: None,
            file: None,
          };
        });

      if child_file_or_dir.dir.is_some() {
        current_dir = child_file_or_dir.dir.unwrap()
      }

      println!("{}", entry.path().display());
    }
  }
  Ok(())
}
