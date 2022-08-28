use rspc::Type;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
	invalidate_query,
	prisma::{file, tag, tag_on_file},
};

use super::{LibraryArgs, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct TagCreateArgs {
	pub name: String,
	pub color: String,
}

#[derive(Type, Deserialize)]
pub struct TagAssignArgs {
	pub file_id: i32,
	pub tag_id: i32,
}

#[derive(Type, Deserialize)]
pub struct TagUpdateArgs {
	pub id: i32,
	pub name: Option<String>,
	pub color: Option<String>,
}

pub(crate) fn mount() -> RouterBuilder {
	RouterBuilder::new()
		.query("get", |ctx, arg: LibraryArgs<()>| async move {
			let (_, library) = arg.get_library(&ctx).await?;

			Ok(library.db.tag().find_many(vec![]).exec().await?)
		})
		.query("getFiles", |ctx, arg: LibraryArgs<i32>| async move {
			let (tag_id, library) = arg.get_library(&ctx).await?;

			Ok(library
				.db
				.file()
				.find_many(vec![file::tags::some(vec![tag_on_file::tag_id::equals(
					tag_id,
				)])])
				.exec()
				.await?)
		})
		.query("getForFile", |ctx, arg: LibraryArgs<i32>| async move {
			let (file_id, library) = arg.get_library(&ctx).await?;

			Ok(library
				.db
				.tag()
				.find_many(vec![tag::tag_files::some(vec![
					tag_on_file::file_id::equals(file_id),
				])])
				.exec()
				.await?)
		})
		.mutation(
			"create",
			|ctx, arg: LibraryArgs<TagCreateArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

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

				invalidate_query!(
					library,
					"tags.get": LibraryArgs<()>,
					LibraryArgs {
						library_id: library.id,
						arg: ()
					}
				);

				Ok(created_tag)
			},
		)
		.mutation(
			"assign",
			|ctx, arg: LibraryArgs<TagAssignArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				library.db.tag_on_file().create(
					tag::id::equals(args.tag_id),
					file::id::equals(args.file_id),
					vec![],
				);

				Ok(())
			},
		)
		.mutation(
			"update",
			|ctx, arg: LibraryArgs<TagUpdateArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				library
					.db
					.tag()
					.update(
						tag::id::equals(args.id),
						vec![tag::name::set(args.name), tag::color::set(args.color)],
					)
					.exec()
					.await?;

				invalidate_query!(
					library,
					"tags.get": LibraryArgs<()>,
					LibraryArgs {
						library_id: library.id,
						arg: ()
					}
				);

				Ok(())
			},
		)
		.mutation("delete", |ctx, arg: LibraryArgs<i32>| async move {
			let (id, library) = arg.get_library(&ctx).await?;

			library.db.tag().delete(tag::id::equals(id)).exec().await?;

			invalidate_query!(
				library,
				"tags.get": LibraryArgs<()>,
				LibraryArgs {
					library_id: library.id,
					arg: ()
				}
			);

			Ok(())
		})
}
