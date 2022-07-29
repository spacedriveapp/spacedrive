use rspc::Type;
use serde::Deserialize;
use uuid::Uuid;

use crate::library::{calculate_statistics, LibraryConfig};

use super::{LibraryArgs, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct EditLibraryArgs {
	pub id: Uuid,
	pub name: Option<String>,
	pub description: Option<String>,
}

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.query("get", |ctx, _: ()| async move {
			ctx.library_manager.get_all_libraries_config().await
		})
		.query("getStatistics", |ctx, arg: LibraryArgs<()>| async move {
			let (_, library) = arg.get_library(&ctx).await?;
			Ok(calculate_statistics(&library).await.unwrap())
		})
		.mutation("create", |ctx, name: String| async move {
			ctx.library_manager
				.create(LibraryConfig {
					name: name.to_string(),
					..Default::default()
				})
				.await
				.unwrap();
		})
		.mutation("edit", |ctx, args: EditLibraryArgs| async move {
			ctx.library_manager
				.edit(args.id, args.name, args.description)
				.await
				.unwrap();
		})
		.mutation("delete", |ctx, id: Uuid| async move {
			ctx.library_manager.delete_library(id).await.unwrap();
		})
}
