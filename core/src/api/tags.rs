use rspc::{ErrorCode, Type};
use serde::Deserialize;
use tracing::info;
use uuid::Uuid;

use crate::{
	api::locations::{object_with_file_paths, ExplorerContext, ExplorerData, ExplorerItem},
	invalidate_query,
	object::preview::THUMBNAIL_CACHE_DIR_NAME,
	prisma::{object, tag, tag_on_object},
};

use super::{utils::LibraryRequest, RouterBuilder};

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.library_query("list", |t| {
			t(
				|_, _: (), library| async move { Ok(library.db.tag().find_many(vec![]).exec().await?) },
			)
		})
		.library_query("getExplorerData", |t| {
			t(|_, tag_id: i32, library| async move {
				info!("Getting files for tag {}", tag_id);

				let tag = library
					.db
					.tag()
					.find_unique(tag::id::equals(tag_id))
					.exec()
					.await?
					.ok_or_else(|| {
						rspc::Error::new(
							ErrorCode::NotFound,
							format!("Tag <id={tag_id}> not found"),
						)
					})?;

				let objects: Vec<ExplorerItem> = library
					.db
					.object()
					.find_many(vec![object::tags::some(vec![
						tag_on_object::tag_id::equals(tag_id),
					])])
					.include(object_with_file_paths::include())
					.exec()
					.await?
					.into_iter()
					.map(|mut object| {
						// sorry brendan
						// grab the first path and tac on the name
						let oldest_path = &object.file_paths[0];
						object.name = Some(oldest_path.name.clone());
						object.extension = oldest_path.extension.clone();
						// a long term fix for this would be to have the indexer give the Object a name and extension, sacrificing its own and only store newly found Path names that differ from the Object name

						let thumb_path = library
							.config()
							.data_directory()
							.join(THUMBNAIL_CACHE_DIR_NAME)
							.join(&object.cas_id)
							.with_extension("webp");

						object.has_thumbnail = thumb_path.try_exists().unwrap();

						ExplorerItem::Object(object)
					})
					.collect();

				info!("Got objects {}", objects.len());

				Ok(ExplorerData {
					context: ExplorerContext::Tag(tag),
					items: objects,
				})
			})
		})
		.library_query("getForObject", |t| {
			t(|_, object_id: i32, library| async move {
				Ok(library
					.db
					.tag()
					.find_many(vec![tag::tag_objects::some(vec![
						tag_on_object::object_id::equals(object_id),
					])])
					.exec()
					.await?)
			})
		})
		.library_query("get", |t| {
			t(|_, tag_id: i32, library| async move {
				Ok(library
					.db
					.tag()
					.find_unique(tag::id::equals(tag_id))
					.exec()
					.await?)
			})
		})
		.library_mutation("create", |t| {
			#[derive(Type, Deserialize)]
			pub struct TagCreateArgs {
				pub name: String,
				pub color: String,
			}

			t(|_, args: TagCreateArgs, library| async move {
				let created_tag = library
					.db
					.tag()
					.create(
						Uuid::new_v4().as_bytes().to_vec(),
						vec![
							tag::name::set(Some(args.name)),
							tag::color::set(Some(args.color)),
						],
					)
					.exec()
					.await?;

				invalidate_query!(library, "tags.list");

				Ok(created_tag)
			})
		})
		.library_mutation("assign", |t| {
			#[derive(Debug, Type, Deserialize)]
			pub struct TagAssignArgs {
				pub object_id: i32,
				pub tag_id: i32,
				pub unassign: bool,
			}

			t(|_, args: TagAssignArgs, library| async move {
				if args.unassign {
					library
						.db
						.tag_on_object()
						.delete(tag_on_object::tag_id_object_id(args.tag_id, args.object_id))
						.exec()
						.await?;
				} else {
					library
						.db
						.tag_on_object()
						.create(
							tag::id::equals(args.tag_id),
							object::id::equals(args.object_id),
							vec![],
						)
						.exec()
						.await?;
				}

				invalidate_query!(library, "tags.getForObject");

				Ok(())
			})
		})
		.library_mutation("update", |t| {
			#[derive(Type, Deserialize)]
			pub struct TagUpdateArgs {
				pub id: i32,
				pub name: Option<String>,
				pub color: Option<String>,
			}

			t(|_, args: TagUpdateArgs, library| async move {
				library
					.db
					.tag()
					.update(
						tag::id::equals(args.id),
						vec![tag::name::set(args.name), tag::color::set(args.color)],
					)
					.exec()
					.await?;

				invalidate_query!(library, "tags.list");

				Ok(())
			})
		})
		.library_mutation("delete", |t| {
			t(|_, tag_id: i32, library| async move {
				library
					.db
					.tag()
					.delete(tag::id::equals(tag_id))
					.exec()
					.await?;

				invalidate_query!(library, "tags.list");

				Ok(())
			})
		})
}
