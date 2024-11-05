use super::{utils::library, Ctx, R};
use rspc::alpha::AlphaRouter;
use serde::Deserialize;
use specta::Type;

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure(
		"list",
		R.with2(library())
			.query(|(node, library), _: ()| async move {
				Ok(library.db.device().find_many(vec![]).exec().await?)
			}),
	)
}
