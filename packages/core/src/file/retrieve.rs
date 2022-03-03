use crate::{
    db,
    prisma::{File, FileData},
};
use anyhow::Result;
use serde::Serialize;

#[derive(Serialize)]
pub struct Directory {
    pub directory: FileData,
    pub contents: Vec<FileData>,
}

pub async fn get_dir_with_contents(path: &str) -> Result<Directory, String> {
    let db = db::get().await?;

    println!("getting files... {:?}", &path);

    // let mut meta_integrity_hash =
    //     create_meta_integrity_hash(path.to_str().unwrap_or_default(), size)?;

    // meta_integrity_hash.truncate(20);

    let directory = match db
        .file()
        .find_unique(File::name().equals(path.into()))
        .exec()
        .await
    {
        Some(file) => file,
        None => return Err("directory_not_found".to_owned()),
    };

    let files = db
        .file()
        .find_many(vec![File::parent_id().equals(directory.id)])
        .exec()
        .await;

    Ok(Directory {
        directory: directory.clone(),
        contents: files,
    })
}

// pub async fn get_directory(path: &str) {
//     // 1. search db for path
//     // 2. get directory shallow
// }
