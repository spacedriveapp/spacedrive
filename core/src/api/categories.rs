use crate::library::{get_category_count, Category};

use std::str::FromStr;

use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;
use strum::VariantNames;

use super::{utils::library, Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("list", {
		#[derive(Type, Deserialize, Serialize)]
		struct CategoryItem {
			name: String,
			count: i32,
		}
		R.with2(library()).query(|(_, library), _: ()| async move {
			let mut category_items = Vec::with_capacity(Category::VARIANTS.len());

			for category_str in Category::VARIANTS {
				let category =
					Category::from_str(category_str).expect("it's alright this category string exists");

				// Convert the category to a CategoryItem and push to vector.
				category_items.push(CategoryItem {
					name: category_str.to_string(),
					count: get_category_count(&library.db, category).await,
				});
			}

			Ok(category_items)
		})
	})
}
