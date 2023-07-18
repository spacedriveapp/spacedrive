use super::{utils::library, Router, R};
use crate::preferences::LibraryPreferences;

pub(crate) fn mount() -> Router {
	R.router()
		.procedure("update", {
			R.with2(library())
				.mutation(|(_, library), args: LibraryPreferences| async move {
					args.write(&library.db).await?;

					Ok(())
				})
		})
		.procedure("get", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(LibraryPreferences::read(&library.db).await?)
			})
		})
}
