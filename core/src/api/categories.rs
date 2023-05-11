use rspc::alpha::AlphaRouter;
use strum::VariantNames;

use crate::library::cat::Category;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("list", {
		R.with2(library()).query(|(_, _library), _: ()| async move {
			// return Category enum as js array
			Ok(Category::VARIANTS.iter().collect::<Vec<_>>())
		})
	})
}
