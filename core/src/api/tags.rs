use rspc::Type;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
	invalidate_query,
	prisma::{file, tag, tag_on_file},
	tag::Tag,
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

			Ok(library
				.db
				.tag()
				.find_many(vec![])
				.exec()
				.await
				.unwrap()
				.into_iter()
				.map(Into::into)
				.collect::<Vec<Tag>>())
		})
		.query("getFilesForTag", |ctx, arg: LibraryArgs<i32>| async move {
			let (tag_id, library) = arg.get_library(&ctx).await?;

			let tag: Option<Tag> = library
				.db
				.tag()
				.find_unique(tag::id::equals(tag_id))
				.exec()
				.await
				.unwrap()
				.map(Into::into);

			Ok(tag)
		})
		.mutation(
			"create",
			|ctx, arg: LibraryArgs<TagCreateArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				let created_tag: Tag = library
					.db
					.tag()
					.create(
						tag::pub_id::set(Uuid::new_v4().as_bytes().to_vec()),
						vec![
							tag::name::set(Some(args.name)),
							tag::color::set(Some(args.color)),
						],
					)
					.exec()
					.await
					.unwrap()
					.into();

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
					tag_on_file::tag::link(tag::id::equals(args.tag_id)),
					tag_on_file::file::link(file::id::equals(args.file_id)),
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
					.find_unique(tag::id::equals(args.id))
					.update(vec![tag::name::set(args.name), tag::color::set(args.color)])
					.exec()
					.await
					.unwrap();

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

			library
				.db
				.tag()
				.find_unique(tag::id::equals(id))
				.delete()
				.exec()
				.await
				.unwrap()
				.unwrap();

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
