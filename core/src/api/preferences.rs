use crate::preferences::LibraryPreferences;

use rspc::alpha::AlphaRouter;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
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
