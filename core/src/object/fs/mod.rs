use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
	job::JobError,
	prisma::{file_path, location, PrismaClient},
};

pub mod decrypt;
pub mod delete;
pub mod encrypt;
pub mod erase;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum ObjectType {
	File,
	Directory,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FsInfo {
	pub obj_id: Option<i32>,
	pub obj_name: String,
	pub obj_path: PathBuf,
	pub obj_type: ObjectType,
}

pub async fn context_menu_fs_info(
	db: &PrismaClient,
	location_id: i32,
	path_id: i32,
) -> Result<FsInfo, JobError> {
	let location = db
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await?
		.ok_or(JobError::MissingData {
			value: String::from("location which matches location_id"),
		})?;

	let item = db
		.file_path()
		.find_unique(file_path::location_id_id(location_id, path_id))
		.exec()
		.await?
		.ok_or(JobError::MissingData {
			value: String::from("file_path that matches both location id and path id"),
		})?;

	let obj_path = [
		location
			.local_path
			.as_ref()
			.map(PathBuf::from)
			.ok_or(JobError::MissingData {
				value: String::from("path when cast as `PathBuf`"),
			})?,
		item.materialized_path.clone().into(),
	]
	.iter()
	.collect();

	// i don't know if this covers symlinks
	let obj_type = if item.is_dir {
		ObjectType::Directory
	} else {
		ObjectType::File
	};

	Ok(FsInfo {
		obj_id: item.object_id,
		obj_name: item.materialized_path.clone(),
		obj_type,
		obj_path,
	})
}

// pub async fn context_menu_all_paths(
// 	db: &PrismaClient,
// 	location_id: i32,
// 	path_id: i32,
// ) -> Result<(), JobError> {
// 	// find all occurances of the same file
// 	// either via cas id or object id, will need to check
// 	// return them as a Vec<PathBuf>
// 	// will be used by `files.allOccurances(location_id, path_id)` route

// 	todo!()
// }
