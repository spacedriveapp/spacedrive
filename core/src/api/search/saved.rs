use chrono::{DateTime, FixedOffset, Utc};
use rspc::alpha::AlphaRouter;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{api::utils::library, invalidate_query, library::Library, prisma::saved_search};

use super::{Ctx, R};

#[derive(Serialize, Type, Deserialize, Clone, Debug)]
pub struct Filter {
	pub value: String,
	pub name: String,
	pub icon: Option<String>,
	pub filter_type: i32,
}

#[derive(Serialize, Type, Deserialize, Clone, Debug)]
pub struct SavedSearchCreateArgs {
	pub name: Option<String>,
	pub filters: Option<Vec<Filter>>,
	pub description: Option<String>,
	pub icon: Option<String>,
}

#[derive(Serialize, Type, Deserialize, Clone, Debug)]
pub struct SavedSearchUpdateArgs {
	pub id: i32,
	pub name: Option<String>,
	pub filters: Option<Vec<Filter>>,
	pub description: Option<String>,
	pub icon: Option<String>,
}

impl SavedSearchCreateArgs {
	pub async fn exec(
		self,
		Library { db, .. }: &Library,
	) -> prisma_client_rust::Result<saved_search::Data> {
		print!("SavedSearchCreateArgs {:?}", self);
		let pub_id = Uuid::new_v4().as_bytes().to_vec();
		let date_created: DateTime<FixedOffset> = Utc::now().into();

		let mut params = vec![saved_search::date_created::set(Some(date_created))];

		if let Some(name) = self.name {
			params.push(saved_search::name::set(Some(name)));
		}

		if let Some(filters) = &self.filters {
			let filters_as_string = serde_json::to_string(filters).unwrap();
			let filters_as_bytes = filters_as_string.into_bytes();
			params.push(saved_search::filters::set(Some(filters_as_bytes)));
		}

		if let Some(description) = self.description {
			params.push(saved_search::description::set(Some(description)));
		}

		if let Some(icon) = self.icon {
			params.push(saved_search::icon::set(Some(icon)));
		}

		db.saved_search().create(pub_id, params).exec().await
	}
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("create", {
			R.with2(library())
				.mutation(|(_, library), args: SavedSearchCreateArgs| async move {
					args.exec(&library).await?;
					invalidate_query!(library, "search.savedSearches.list");
					Ok(())
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
			#[derive(Serialize, Type, Deserialize, Clone)]
			pub struct SavedSearchResponse {
				pub id: i32,
				pub pub_id: Vec<u8>,
				pub name: Option<String>,
				pub icon: Option<String>,
				pub description: Option<String>,
				pub order: Option<i32>,
				pub date_created: Option<DateTime<FixedOffset>>,
				pub date_modified: Option<DateTime<FixedOffset>>,
				pub filters: Option<Vec<Filter>>,
			}
			R.with2(library()).query(|(_, library), _: ()| async move {
				let searches: Vec<saved_search::Data> = library
					.db
					.saved_search()
					.find_many(vec![])
					// .order_by(saved_search::order::order(prisma::SortOrder::Desc))
					.exec()
					.await?;
				let result: Result<Vec<SavedSearchResponse>, _> = searches
					.into_iter()
					.map(|search| {
						let filters_bytes = search.filters.unwrap_or_else(Vec::new);

						let filters_string = String::from_utf8(filters_bytes).unwrap();
						let filters: Vec<Filter> = serde_json::from_str(&filters_string).unwrap();

						Ok(SavedSearchResponse {
							id: search.id,
							pub_id: search.pub_id,
							name: search.name,
							icon: search.icon,
							description: search.description,
							order: search.order,
							date_created: search.date_created,
							date_modified: search.date_modified,
							filters: Some(filters),
						})
					})
					.collect(); // Collects the Result, if there is any Err it will be propagated.

				result
			})
		})
		.procedure("update", {
			R.with2(library())
				.mutation(|(_, library), args: SavedSearchUpdateArgs| async move {
					let mut params = vec![];

					if let Some(name) = args.name {
						params.push(saved_search::name::set(Some(name)));
					}

					if let Some(filters) = &args.filters {
						let filters_as_string = serde_json::to_string(filters).unwrap();
						let filters_as_bytes = filters_as_string.into_bytes();
						params.push(saved_search::filters::set(Some(filters_as_bytes)));
					}

					if let Some(description) = args.description {
						params.push(saved_search::description::set(Some(description)));
					}

					if let Some(icon) = args.icon {
						params.push(saved_search::icon::set(Some(icon)));
					}

					params.push(saved_search::date_modified::set(Some(Utc::now().into())));

					library
						.db
						.saved_search()
						.update(saved_search::id::equals(args.id), params)
						.exec()
						.await?;

					invalidate_query!(library, "search.savedSearches.list");

					Ok(())
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
					invalidate_query!(library, "search.savedSearches.list");
					Ok(())
				})
		})
}
