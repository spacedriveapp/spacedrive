use std::vec;

use crate::library::{get_category_count, Category};

use super::{utils::library, Ctx, R};

use futures::executor::block_on;
use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::str::FromStr;
use strum::VariantNames;

// static array of searchable categories

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("list", {
		#[derive(Type, Deserialize, Serialize)]
		struct CategoryItem {
			name: String,
			count: i32,
		}
		R.with2(library()).query(|(_, library), _: ()| async move {
			let mut category_items = Vec::new();

			for category_str in Category::VARIANTS.into_iter() {
				let category = Category::from_str(&category_str).unwrap();

				// Fetch the count for the category.
				let count = block_on(get_category_count(&library.db, category));

				// Convert the category to a CategoryItem and push to vector.
				category_items.push(CategoryItem {
					name: category_str.to_string(),
					count,
				});
			}

			Ok(category_items)
		})
	})
}
