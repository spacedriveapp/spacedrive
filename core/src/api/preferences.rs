use super::{utils::library, RouterBuilder, R};
use crate::preferences::LibraryPreferences;

pub(crate) fn mount() -> RouterBuilder {
	R.router()
		.procedure("update", {
			R.with(library())
				.mutation(|(_, library), args: LibraryPreferences| async move {
					args.write(&library.db).await?;

					Ok(())
				})
		})
		.procedure("get", {
			R.with(library()).query(|(_, library), _: ()| async move {
				Ok(LibraryPreferences::read(&library.db).await?)
			})
		})
}
