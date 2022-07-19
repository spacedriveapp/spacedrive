use std::path::PathBuf;

use serde::Deserialize;
use ts_rs::TS;

use crate::{file::explorer, library::Statistics, prisma::location, sys};

use super::{LibraryRouter, LibraryRouterBuilder};

#[derive(TS, Deserialize)]
pub struct LocationUpdateArgs {
	pub id: i32,
	pub name: Option<String>,
}

#[derive(TS, Deserialize)]
pub struct GetExplorerDirArgs {
	pub location_id: i32,
	pub path: PathBuf,
	pub limit: i32,
}

pub(crate) fn mount() -> LibraryRouterBuilder {
	<LibraryRouter>::new()
		.query("get", |ctx, _: ()| async move {
			sys::get_locations(&ctx.library).await.unwrap()
		})
		.query("getById", |ctx, id: i32| async move {
			sys::get_location(&ctx.library, id).await.unwrap()
		})
		.query(
			"getExplorerDir",
			|ctx, args: GetExplorerDirArgs| async move {
				explorer::open_dir(&ctx.library, args.location_id, args.path)
					.await
					.unwrap()
			},
		)
		.query("getStatistics", |ctx, _: ()| async move {
			Statistics::calculate(&ctx.library).await.unwrap()
		})
		.mutation("create", |ctx, path: PathBuf| async move {
			sys::new_location_and_scan(&ctx.library, &path)
				.await
				.unwrap()
		})
		.mutation("update", |ctx, args: LocationUpdateArgs| async move {
			ctx.library
				.db
				.location()
				.find_unique(location::id::equals(args.id))
				.update(vec![location::name::set(args.name)])
				.exec()
				.await
				.unwrap();
		})
		.mutation("delete", |ctx, id: i32| async move {
			sys::delete_location(&ctx.library, id).await.unwrap();
		})
		.mutation("fullRescan", |ctx, id: i32| async move {
			sys::scan_location(&ctx.library, id, String::new()).await;
		})
		.mutation("quickRescan", |_, _: ()| todo!())
}
