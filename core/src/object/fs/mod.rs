pub mod create;

use crate::{
	job::JobError,
	prisma::{file_path, location, PrismaClient},
};

use std::{ffi::OsStr, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::preview::file_path_with_object;

pub mod copy;
pub mod cut;

pub mod decrypt;
pub mod delete;
pub mod encrypt;

pub mod error;

pub mod erase;

// pub const BYTES: &str = "bytes";

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum ObjectType {
	File,
	Directory,
}

pub const BYTES_EXT: &str = ".bytes";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FsInfo {
	pub path_data: file_path_with_object::Data,
	pub fs_path: PathBuf,
}

pub fn osstr_to_string(os_str: Option<&OsStr>) -> Result<String, JobError> {
	os_str
		.and_then(OsStr::to_str)
		.map(str::to_string)
		.ok_or(JobError::OsStr)
}

pub async fn get_path_from_location_id(
	db: &PrismaClient,
	location_id: i32,
) -> Result<PathBuf, JobError> {
	Ok(db
		.location()
		.find_unique(location::id::equals(location_id))
		.exec()
		.await?
		.ok_or(JobError::MissingData {
			value: String::from("location which matches location_id"),
		})?
		.path
		.into())
}

pub async fn context_menu_fs_info(
	db: &PrismaClient,
	location_id: i32,
	path_id: i32,
) -> Result<FsInfo, JobError> {
	let path_data = db
		.file_path()
		.find_unique(file_path::location_id_id(location_id, path_id))
		.include(file_path_with_object::include())
		.exec()
		.await?
		.ok_or(JobError::MissingData {
			value: String::from("file_path that matches both location id and path id"),
		})?;

	Ok(FsInfo {
		fs_path: get_path_from_location_id(db, location_id)
			.await?
			.join(&path_data.materialized_path),
		path_data,
	})
}
