use rspc::alpha::AlphaRouter;

use super::{utils::library, Ctx, R};
use crate::preferences::LibraryPreferences;

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("update", {
		R.with2(library())
			.mutation(|(_, library), args: LibraryPreferences| async move {
				args.write(&library.db).await?;

				Ok(())
			})
	})
}
