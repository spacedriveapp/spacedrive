use serde::Deserialize;
use ts_rs::TS;

use crate::library::LibraryConfig;

use super::{Router, RouterBuilder};

#[derive(TS, Deserialize)]
pub struct EditLibraryArgs {
	pub id: String,
	pub name: Option<String>,
	pub description: Option<String>,
}

pub(crate) fn mount() -> RouterBuilder {
	<Router>::new()
		.query("get", |ctx, _: ()| async move {
			ctx.library_manager.get_all_libraries_config().await
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
		.mutation("delete", |ctx, id: String| async move {
			ctx.library_manager.delete_library(id).await.unwrap();
		})
}
