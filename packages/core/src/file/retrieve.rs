use crate::{db, prisma::File};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{FileError, FileResource};

#[derive(Serialize, Deserialize, TS, Debug)]
#[ts(export)]
pub struct Directory {
	pub directory: FileResource,
	pub contents: Vec<FileResource>,
}

pub async fn get_dir_with_contents(path: &str) -> Result<Directory, FileError> {
	let db = db::get().await?;

	println!("getting files... {:?}", &path);

	let directory = db
		.file()
		.find_unique(File::name().equals(path.into()))
		.exec()
		.await
		.ok_or(FileError::FileNotFound(path.to_string()))?;

	let files = db.file().find_many(vec![File::parent_id().equals(directory.id)]).exec().await;

	Ok(Directory {
		directory: directory.into(),
		contents: files.into_iter().map(|l| l.into()).collect(),
	})
}
