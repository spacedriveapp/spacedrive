use crate::library::Category;

use std::str::FromStr;

use rspc::alpha::AlphaRouter;
use strum::VariantNames;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("list", {
		R.with2(library()).query(|(_, library), _: ()| async move {
			let (categories, queries): (Vec<_>, Vec<_>) = Category::VARIANTS
				.iter()
				.map(|category| {
					let category = Category::from_str(category)
						.expect("it's alright this category string exists");
					(
						category,
						library.db.object().count(vec![category.to_where_param()]),
					)
				})
				.unzip();

			Ok(library
				.db
				._batch(queries)
				.await?
				.into_iter()
				.zip(categories)
				.collect::<Vec<_>>())
		})
	})
}
