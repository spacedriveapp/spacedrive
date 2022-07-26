use std::path::PathBuf;

use rspc::Type;
use serde::{Deserialize, Serialize};

use crate::{file::explorer, library::Statistics, prisma::location, sys};

use super::{LibraryArgs, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct LocationUpdateArgs {
	pub id: i32,
	pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct GetExplorerDirArgs {
	pub location_id: i32,
	pub path: PathBuf,
	pub limit: i32,
}

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.query("get", |ctx, arg: LibraryArgs<()>| async move {
			let (_, library) = arg.get_library(&ctx).await?;

			Ok(sys::get_locations(&library).await.unwrap())
		})
		.query("getById", |ctx, arg: LibraryArgs<i32>| async move {
			let (id, library) = arg.get_library(&ctx).await?;

			Ok(sys::get_location(&library, id).await.unwrap())
		})
		.query(
			"getExplorerDir",
			|ctx, arg: LibraryArgs<GetExplorerDirArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				Ok(explorer::open_dir(&library, args.location_id, args.path)
					.await
					.unwrap())
			},
		)
		.query("getStatistics", |ctx, arg: LibraryArgs<()>| async move {
			let (args, library) = arg.get_library(&ctx).await?;

			Ok(Statistics::calculate(&library).await.unwrap())
		})
		.mutation("create", |ctx, arg: LibraryArgs<PathBuf>| async move {
			let (path, library) = arg.get_library(&ctx).await?;

			Ok(sys::new_location_and_scan(&library, &path).await.unwrap())
		})
		.mutation(
			"update",
			|ctx, arg: LibraryArgs<LocationUpdateArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				library
					.db
					.location()
					.find_unique(location::id::equals(args.id))
					.update(vec![location::name::set(args.name)])
					.exec()
					.await
					.unwrap();

				Ok(())
			},
		)
		.mutation("delete", |ctx, arg: LibraryArgs<i32>| async move {
			let (id, library) = arg.get_library(&ctx).await?;

			Ok(sys::delete_location(&library, id).await.unwrap())
		})
		.mutation("fullRescan", |ctx, arg: LibraryArgs<i32>| async move {
			let (id, library) = arg.get_library(&ctx).await?;

			sys::scan_location(&library, id, String::new()).await;

			Ok(())
		})
		.mutation("quickRescan", |_, _: LibraryArgs<()>| todo!())
}
