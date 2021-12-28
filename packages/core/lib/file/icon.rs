

pub async fn get_thumbs_for_directory(window: tauri::Window, path: &str) -> Result<(), String> {
  let config = config::get_config();
  let dir = filesystem::retrieve::get_dir_with_contents(&path).await?;
  // iterate over directory contents
  for file in dir.contents.into_iter() {
    let now = Instant::now();
    let icon_name = format!(
      "{}.png",
      if file.is_dir {
        "folder".to_owned()
      } else {
        file.extension
      }
    );
    let icon_path = config.file_type_thumb_dir.join(icon_name);
    // extract metadata from file
    let existing = fs::metadata(&icon_path).is_ok();
    // write thumbnail only if
    if !existing {
      // call swift to get thumbnail data
      let thumbnail_b64 = get_file_thumbnail_base64(&file.uri).to_string();
      fs::write(
        &icon_path,
        base64::decode(thumbnail_b64).unwrap_or_default(),
      )
      .map_err(|_| "thumb_cache_failure")?;
    }
    println!("cached thumb {:?} in {:?}", file.id, now.elapsed());

    if !existing {
      reply(
        &window,
        GlobalEventKind::FileTypeThumb,
        GenFileTypeIconsResponse {
          icon_created: true,
          file_id: file.id,
        },
      )
    }
  }

  Ok(())
}