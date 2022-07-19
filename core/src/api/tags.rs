use serde::Deserialize;
use ts_rs::TS;

use crate::{
	prisma::{file, tag, tag_on_file},
	tag::Tag,
};

use super::{LibraryRouter, LibraryRouterBuilder};

#[derive(TS, Deserialize)]
pub struct TagCreateArgs {
	pub name: String,
	pub color: String,
}

#[derive(TS, Deserialize)]
pub struct TagAssignArgs {
	pub file_id: i32,
	pub tag_id: i32,
}

#[derive(TS, Deserialize)]
pub struct TagUpdateArgs {
	pub id: i32,
	pub name: Option<String>,
	pub color: Option<String>,
}

pub(crate) fn mount() -> LibraryRouterBuilder {
	<LibraryRouter>::new()
		.query("get", |ctx, _: ()| async move {
			ctx.library
				.db
				.tag()
				.find_many(vec![])
				.exec()
				.await
				.unwrap()
				.into_iter()
				.map(Into::into)
				.collect::<Vec<Tag>>()
		})
		.query("getFilesForTag", |ctx, tag_id: i32| async move {
			let tag: Option<Tag> = ctx
				.library
				.db
				.tag()
				.find_unique(tag::id::equals(tag_id))
				.exec()
				.await
				.unwrap()
				.map(Into::into);

			tag
		})
		.mutation("create", |ctx, args: TagCreateArgs| async move {
			let created_tag: Tag = ctx
				.library
				.db
				.tag()
				.create(
					tag::pub_id::set(uuid::Uuid::new_v4().to_string()),
					vec![
						tag::name::set(Some(args.name)),
						tag::color::set(Some(args.color)),
					],
				)
				.exec()
				.await
				.unwrap()
				.into();

			// ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
			// 	library_id: ctx.id.to_string(),
			// 	query: LibraryQuery::GetTags,
			// }))
			// .await;

			created_tag
		})
		.mutation("assign", |ctx, args: TagAssignArgs| async move {
			ctx.library.db.tag_on_file().create(
				tag_on_file::tag::link(tag::id::equals(args.tag_id)),
				tag_on_file::file::link(file::id::equals(args.file_id)),
				vec![],
			);
		})
		.mutation("update", |ctx, args: TagUpdateArgs| async move {
			ctx.library
				.db
				.tag()
				.find_unique(tag::id::equals(args.id))
				.update(vec![tag::name::set(args.name), tag::color::set(args.color)])
				.exec()
				.await
				.unwrap();

			// ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
			// 	library_id: ctx.id.to_string(),
			// 	query: LibraryQuery::GetTags,
			// }))
			// .await;
		})
		.mutation("delete", |ctx, id: i32| async move {
			ctx.library
				.db
				.tag()
				.find_unique(tag::id::equals(id))
				.delete()
				.exec()
				.await
				.unwrap()
				.unwrap();

			// ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
			// 	library_id: ctx.id.to_string(),
			// 	query: LibraryQuery::GetTags,
			// }))
			// .await;
		})
}
