use serde::Deserialize;
use ts_rs::TS;

use crate::prisma::file;

use super::{LibraryRouter, LibraryRouterBuilder};

#[derive(TS, Deserialize)]
pub struct SetNoteArgs {
	pub id: i32,
	pub note: Option<String>,
}

#[derive(TS, Deserialize)]
pub struct SetFavoriteArgs {
	pub id: i32,
	pub favorite: bool,
}

pub(crate) fn mount() -> LibraryRouterBuilder {
	<LibraryRouter>::new()
		.query("readMetadata", |_ctx, _id: i32| todo!())
		.mutation("setNote", |ctx, args: SetNoteArgs| async move {
			ctx.library
				.db
				.file()
				.find_unique(file::id::equals(args.id))
				.update(vec![file::note::set(args.note)])
				.exec()
				.await
				.unwrap();

			// ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
			// 	library_id: ctx.id.to_string(),
			// 	query: LibraryQuery::GetExplorerDir {
			// 		limit: 0,
			// 		path: PathBuf::new(),
			// 		location_id: 0,
			// 	},
			// }))
			// .await;
		})
		.mutation("setFavorite", |ctx, args: SetFavoriteArgs| async move {
			ctx.library
				.db
				.file()
				.find_unique(file::id::equals(args.id))
				.update(vec![file::favorite::set(args.favorite)])
				.exec()
				.await
				.unwrap();

			// ctx.emit(CoreEvent::InvalidateQuery(ClientQuery::LibraryQuery {
			// 	library_id: ctx.id.to_string(),
			// 	query: LibraryQuery::GetExplorerDir {
			// 		limit: 0,
			// 		path: PathBuf::new(),
			// 		location_id: 0,
			// 	},
			// }))
			// .await;
		})
		.mutation("delete", |ctx, id: i32| async move {
			ctx.library
				.db
				.file()
				.find_unique(file::id::equals(id))
				.delete()
				.exec()
				.await
				.unwrap();
		})
}
