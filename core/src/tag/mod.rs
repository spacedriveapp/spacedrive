use crate::{
	file::File,
	library::LibraryContext,
	prisma::{
		self, file,
		tag::{self},
		tag_on_file,
	},
	ClientQuery, CoreError, CoreEvent, CoreResponse, LibraryQuery,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Tag {
	pub id: i32,
	pub pub_id: String,
	pub name: Option<String>,
	pub color: Option<String>,

	pub total_files: Option<i32>,
	pub redundancy_goal: Option<i32>,

	pub date_created: chrono::DateTime<chrono::Utc>,
	pub date_modified: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
			pub_id: data.pub_id,
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

#[derive(Serialize, Deserialize, TS, Debug)]
#[ts(export)]
pub struct TagWithFiles {
	pub tag: Tag,
	pub files_with_tag: Vec<TagOnFile>,
}

#[derive(Error, Debug)]
pub enum TagError {
	#[error("Tag not found")]
	TagNotFound(i32),
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
}

pub async fn create_tag(
	ctx: LibraryContext,
	name: String,
	color: String,
) -> Result<CoreResponse, CoreError> {
	let created_tag = ctx
		.db
		.tag()
		.create(
			tag::pub_id::set(uuid::Uuid::new_v4().to_string()),
			vec![tag::name::set(Some(name)), tag::color::set(Some(color))],
		)
		.exec()
		.await
		.unwrap();

	ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
		library_id: ctx.id.to_string(),
		query: LibraryQuery::GetTags,
	}))
	.await;

	Ok(CoreResponse::TagCreateResponse(created_tag.into()))
}

pub async fn update_tag(
	ctx: LibraryContext,
	id: i32,
	name: Option<String>,
	color: Option<String>,
) -> Result<CoreResponse, CoreError> {
	ctx.db
		.tag()
		.find_unique(tag::id::equals(id))
		.update(vec![tag::name::set(name), tag::color::set(color)])
		.exec()
		.await
		.unwrap();

	ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
		library_id: ctx.id.to_string(),
		query: LibraryQuery::GetTags,
	}))
	.await;

	Ok(CoreResponse::Success(()))
}

pub async fn tag_assign(
	ctx: LibraryContext,
	file_id: i32,
	tag_id: i32,
) -> Result<CoreResponse, CoreError> {
	ctx.db.tag_on_file().create(
		tag_on_file::tag::link(tag::UniqueWhereParam::IdEquals(tag_id)),
		tag_on_file::file::link(file::UniqueWhereParam::IdEquals(file_id)),
		vec![],
	);

	Ok(CoreResponse::Success(()))
}

pub async fn tag_delete(ctx: LibraryContext, id: i32) -> Result<CoreResponse, CoreError> {
	ctx.db
		.tag()
		.find_unique(tag::id::equals(id))
		.delete()
		.exec()
		.await?
		.unwrap();

	ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
		library_id: ctx.id.to_string(),
		query: LibraryQuery::GetTags,
	}))
	.await;

	Ok(CoreResponse::Success(()))
}

pub async fn get_files_for_tag(ctx: LibraryContext, id: i32) -> Result<CoreResponse, CoreError> {
	let tag: Option<Tag> = ctx
		.db
		.tag()
		.find_unique(tag::id::equals(id))
		.exec()
		.await?
		.map(Into::into);

	Ok(CoreResponse::GetTag(tag))
}

pub async fn get_all_tags(ctx: LibraryContext) -> Result<CoreResponse, CoreError> {
	let tags: Vec<Tag> = ctx
		.db
		.tag()
		.find_many(vec![])
		.exec()
		.await?
		.into_iter()
		.map(Into::into)
		.collect();

	Ok(CoreResponse::GetTags(tags))
}
