use serde::Deserialize;
use ts_rs::TS;

use crate::{file, prisma::file as prisma_file};

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
			file::set_note(&ctx.library, args.id, args.note)
				.await
				.unwrap();
		})
		.mutation("setFavorite", |ctx, args: SetFavoriteArgs| async move {
			file::favorite(&ctx.library, args.id, args.favorite)
				.await
				.unwrap();
		})
		.mutation("delete", |ctx, id: i32| async move {
			ctx.library
				.db
				.file()
				.find_unique(prisma_file::id::equals(id))
				.delete()
				.exec()
				.await
				.unwrap();
		})
}
