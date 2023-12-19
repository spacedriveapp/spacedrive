use crate::{api::utils::library, invalidate_query};

use sd_prisma::prisma::saved_search;
use sd_utils::chain_optional_iter;

use chrono::{DateTime, FixedOffset, Utc};
use rspc::alpha::AlphaRouter;
use serde::{de::IgnoredAny, Deserialize, Serialize};
use specta::Type;
use tracing::error;
use uuid::Uuid;

use super::{Ctx, R};

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("create", {
			R.with2(library()).mutation({
				#[derive(Serialize, Type, Deserialize, Clone, Debug)]
				#[specta(inline)]
				pub struct Args {
					pub name: String,
					#[specta(optional)]
					pub search: Option<String>,
					#[specta(optional)]
					pub filters: Option<String>,
					#[specta(optional)]
					pub description: Option<String>,
					#[specta(optional)]
					pub icon: Option<String>,
				}

				|(_, library), args: Args| async move {
					let pub_id = Uuid::new_v4().as_bytes().to_vec();
					let date_created: DateTime<FixedOffset> = Utc::now().into();

					library
						.db
						.saved_search()
						.create(
							pub_id,
							chain_optional_iter(
								[
									saved_search::date_created::set(Some(date_created)),
									saved_search::name::set(Some(args.name)),
								],
								[
									args.filters
										.map(|s| {
											// https://github.com/serde-rs/json/issues/579
											// https://docs.rs/serde/latest/serde/de/struct.IgnoredAny.html
											if let Err(e) = serde_json::from_str::<IgnoredAny>(&s) {
												error!("failed to parse filters: {e:#?}");
												None
											} else {
												Some(s)
											}
										})
										.map(saved_search::filters::set),
									args.search.map(Some).map(saved_search::search::set),
									args.description
										.map(Some)
										.map(saved_search::description::set),
									args.icon.map(Some).map(saved_search::icon::set),
								],
							),
						)
						.exec()
						.await?;

					invalidate_query!(library, "search.saved.list");

					Ok(())
				}
			})
		})
		.procedure("get", {
			R.with2(library())
				.query(|(_, library), search_id: i32| async move {
					Ok(library
						.db
						.saved_search()
						.find_unique(saved_search::id::equals(search_id))
						.exec()
						.await?)
				})
		})
		.procedure("list", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(library
					.db
					.saved_search()
					.find_many(vec![])
					// .order_by(saved_search::order::order(prisma::SortOrder::Desc))
					.exec()
					.await?)
			})
		})
		.procedure("update", {
			R.with2(library()).mutation({
				saved_search::partial_unchecked!(Args {
					name
					description
					icon
					search
					filters
				});

				|(_, library), (id, args): (saved_search::id::Type, Args)| async move {
					let mut params = args.to_params();
					params.push(saved_search::date_modified::set(Some(Utc::now().into())));

					library
						.db
						.saved_search()
						.update_unchecked(saved_search::id::equals(id), params)
						.exec()
						.await?;

					invalidate_query!(library, "search.saved.list");
					invalidate_query!(library, "search.saved.get");

					Ok(())
				}
			})
		})
		.procedure("delete", {
			R.with2(library())
				.mutation(|(_, library), search_id: i32| async move {
					library
						.db
						.saved_search()
						.delete(saved_search::id::equals(search_id))
						.exec()
						.await?;

					invalidate_query!(library, "search.saved.list");
					// disabled as it's messing with pre-delete navigation
					// invalidate_query!(library, "search.saved.get");

					Ok(())
				})
		})
}
