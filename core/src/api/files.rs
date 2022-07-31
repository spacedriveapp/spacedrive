use rspc::Type;
use serde::Deserialize;

use crate::{api::locations::GetExplorerDirArgs, invalidate_query, prisma::file};

use super::{LibraryArgs, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct SetNoteArgs {
	pub id: i32,
	pub note: Option<String>,
}

#[derive(Type, Deserialize)]
pub struct SetFavoriteArgs {
	pub id: i32,
	pub favorite: bool,
}

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.query("readMetadata", |_ctx, _id: LibraryArgs<i32>| todo!())
		.mutation("setNote", |ctx, arg: LibraryArgs<SetNoteArgs>| async move {
			let (args, library) = arg.get_library(&ctx).await?;

			library
				.db
				.file()
				.find_unique(file::id::equals(args.id))
				.update(vec![file::note::set(args.note)])
				.exec()
				.await?;

			invalidate_query!(
				library,
				"locations.getExplorerDir": LibraryArgs<GetExplorerDirArgs>,
				LibraryArgs {
					library_id: library.id,
					arg: GetExplorerDirArgs {
						location_id: 0, // TODO: This should be the correct location_id
						path: "".into(),
						limit: 0,
					}
				}
			);

			Ok(())
		})
		.mutation(
			"setFavorite",
			|ctx, arg: LibraryArgs<SetFavoriteArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				library
					.db
					.file()
					.find_unique(file::id::equals(args.id))
					.update(vec![file::favorite::set(args.favorite)])
					.exec()
					.await?;

				invalidate_query!(
					library,
					"locations.getExplorerDir": LibraryArgs<GetExplorerDirArgs>,
					LibraryArgs {
						library_id: library.id,
						arg: GetExplorerDirArgs {
							// TODO: Set these arguments to the correct type
							location_id: 0,
							path: "".into(),
							limit: 0,
						}
					}
				);

				Ok(())
			},
		)
		.mutation("delete", |ctx, arg: LibraryArgs<i32>| async move {
			let (id, library) = arg.get_library(&ctx).await?;

			library
				.db
				.file()
				.find_unique(file::id::equals(id))
				.delete()
				.exec()
				.await?;

			invalidate_query!(
				library,
				"locations.getExplorerDir": LibraryArgs<GetExplorerDirArgs>,
				LibraryArgs {
					library_id: library.id,
					arg: GetExplorerDirArgs {
						// TODO: Set these arguments to the correct type
						location_id: 0,
						path: "".into(),
						limit: 0,
					}
				}
			);
			Ok(())
		})
}
