// TODO: Ensure this file has normalised caching setup before reenabling

// use crate::library::Category;

// use std::{collections::BTreeMap, str::FromStr};

// use rspc::{alpha::AlphaRouter, ErrorCode};
// use strum::VariantNames;

// use super::{utils::library, Ctx, R};

// pub(crate) fn mount() -> AlphaRouter<Ctx> {
// 	R.router().procedure("list", {
// 		R.with2(library()).query(|(_, library), _: ()| async move {
// 			let (categories, queries): (Vec<_>, Vec<_>) = Category::VARIANTS
// 				.iter()
// 				.map(|category| {
// 					let category = Category::from_str(category)
// 						.expect("it's alright this category string exists");
// 					(
// 						category,
// 						library.db.object().count(vec![category.to_where_param()]),
// 					)
// 				})
// 				.unzip();

// 			Ok(categories
// 				.into_iter()
// 				.zip(
// 					library
// 						.db
// 						._batch(queries)
// 						.await?
// 						.into_iter()
// 						// TODO(@Oscar): rspc bigint support
// 						.map(|count| {
// 							i32::try_from(count).map_err(|_| {
// 								rspc::Error::new(
// 									ErrorCode::InternalServerError,
// 									"category item count overflowed 'i32'!".into(),
// 								)
// 							})
// 						})
// 						.collect::<Result<Vec<_>, _>>()?,
// 				)
// 				.collect::<BTreeMap<_, _>>())
// 		})
// 	})
// }
