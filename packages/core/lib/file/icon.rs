use crate::{file::retrieve, get_core_config, native, ClientEvent};

use futures::{
    stream::{self, StreamExt},
    Stream,
};
use std::fs;

pub async fn get_thumbs_for_directory(path: &str) -> impl Stream<Item = ClientEvent> {
    let dir = retrieve::get_dir_with_contents(&path).await.unwrap();

    stream::iter(dir.contents.into_iter()).filter_map(|file| async {
        let config = get_core_config();
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
            let thumbnail_b64 = native::methods::get_file_thumbnail_base64(&file.uri).to_string();
            fs::write(
                &icon_path,
                base64::decode(thumbnail_b64).unwrap_or_default(),
            )
            .unwrap();
        }

        if !existing {
            Some(ClientEvent::NewFileTypeThumb {
                icon_created: true,
                file_id: file.id,
            })
        } else {
            None
        }
    })
}
