use crate::{
	file::File,
	prisma::{tag, tag_on_file},
};
use rspc::Type;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Tag {
	pub id: i32,
	pub pub_id: Uuid,
	pub name: Option<String>,
	pub color: Option<String>,

	pub total_files: Option<i32>,
	pub redundancy_goal: Option<i32>,
	pub date_created: chrono::DateTime<chrono::Utc>,
	pub date_modified: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TagOnFile {
	pub tag_id: i32,
	pub tag: Option<Tag>,

	pub file_id: i32,
	pub file: Option<File>,

	pub date_created: chrono::DateTime<chrono::Utc>,
}

impl From<tag::Data> for Tag {
	fn from(data: tag::Data) -> Self {
		Self {
			id: data.id,
			pub_id: Uuid::from_slice(&data.pub_id).unwrap(),
			name: data.name,
			color: data.color,
			total_files: data.total_files,
			redundancy_goal: data.redundancy_goal,
			date_created: data.date_created.into(),
			date_modified: data.date_modified.into(),
		}
	}
}

impl From<tag_on_file::Data> for TagOnFile {
	fn from(data: tag_on_file::Data) -> Self {
		Self {
			tag_id: data.tag_id,
			tag: data.tag.map(|t| (*t).into()),
			file_id: data.file_id,
			file: data.file.map(|f| (*f).into()),
			date_created: data.date_created.into(),
		}
	}
}

#[derive(Serialize, Deserialize, Type, Debug)]
pub struct TagWithFiles {
	pub tag: Tag,
	pub files_with_tag: Vec<TagOnFile>,
}
