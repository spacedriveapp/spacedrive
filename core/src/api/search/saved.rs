use std::str::FromStr;

use crate::{api::utils::library, invalidate_query, library::Library};

use sd_prisma::{prisma::saved_search, prisma_sync};
use sd_sync::{option_sync_db_entry, sync_db_entry, OperationFactory};
use sd_utils::chain_optional_iter;

use chrono::{DateTime, FixedOffset, Utc};
use rspc::alpha::AlphaRouter;
use serde::{de::IgnoredAny, Deserialize};
use specta::Type;
use tracing::error;
use uuid::Uuid;

use super::{Ctx, R};

#[derive(Type, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
enum SearchTarget {
	#[default]
	Paths,
	Objects,
}

impl std::fmt::Display for SearchTarget {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SearchTarget::Paths => write!(f, "paths"),
			SearchTarget::Objects => write!(f, "objects"),
		}
	}
}

impl FromStr for SearchTarget {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"paths" => Ok(SearchTarget::Paths),
			"objects" => Ok(SearchTarget::Objects),
			_ => Err(format!("invalid search target: {s}")),
		}
	}
}

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("create", {
			R.with2(library()).mutation({
				#[derive(Type, Deserialize, Clone, Debug)]
				#[specta(inline)]
				pub struct Args {
					pub name: String,
					#[serde(default)]
					pub target: SearchTarget,
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
					let Library { db, sync, .. } = library.as_ref();
					let pub_id = Uuid::now_v7().as_bytes().to_vec();
					let date_created: DateTime<FixedOffset> = Utc::now().into();

					let (sync_params, db_params) = chain_optional_iter(
						[
							sync_db_entry!(date_created, saved_search::date_created),
							sync_db_entry!(args.name, saved_search::name),
							sync_db_entry!(args.target.to_string(), saved_search::target),
						],
						[
							option_sync_db_entry!(
								args.filters.and_then(|s| {
									// https://github.com/serde-rs/json/issues/579
									// https://docs.rs/serde/latest/serde/de/struct.IgnoredAny.html

									if let Err(e) = serde_json::from_str::<IgnoredAny>(&s) {
										error!(?e, "Failed to parse filters;");
										None
									} else {
										Some(s)
									}
								}),
								saved_search::filters
							),
							option_sync_db_entry!(args.search, saved_search::search),
							option_sync_db_entry!(args.description, saved_search::description),
							option_sync_db_entry!(args.icon, saved_search::icon),
						],
					)
					.into_iter()
					.unzip::<_, _, Vec<_>, Vec<_>>();

					sync.write_op(
						db,
						sync.shared_create(
							prisma_sync::saved_search::SyncId {
								pub_id: pub_id.clone(),
							},
							sync_params,
						),
						db.saved_search()
							.create(pub_id, db_params)
							.select(saved_search::select!({ id })),
					)
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
					let Library { db, sync, .. } = library.as_ref();
					let updated_at = Utc::now();

					let search = db
						.saved_search()
						.find_unique(saved_search::id::equals(id))
						.select(saved_search::select!({ pub_id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(rspc::ErrorCode::NotFound, "search not found".into())
						})?;

					let (sync_params, db_params) = chain_optional_iter(
						[sync_db_entry!(updated_at, saved_search::date_modified)],
						[
							option_sync_db_entry!(args.name.flatten(), saved_search::name),
							option_sync_db_entry!(args.description.flatten(), saved_search::name),
							option_sync_db_entry!(args.icon.flatten(), saved_search::icon),
							option_sync_db_entry!(args.search.flatten(), saved_search::search),
							option_sync_db_entry!(args.filters.flatten(), saved_search::filters),
						],
					)
					.into_iter()
					.unzip::<_, _, Vec<_>, Vec<_>>();

					sync.write_op(
						db,
						sync.shared_update(
							prisma_sync::saved_search::SyncId {
								pub_id: search.pub_id.clone(),
							},
							sync_params,
						),
						db.saved_search()
							.update_unchecked(saved_search::id::equals(id), db_params),
					)
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
					let Library { db, sync, .. } = library.as_ref();

					let search = db
						.saved_search()
						.find_unique(saved_search::id::equals(search_id))
						.select(saved_search::select!({ pub_id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(rspc::ErrorCode::NotFound, "search not found".into())
						})?;

					sync.write_op(
						db,
						sync.shared_delete(prisma_sync::saved_search::SyncId {
							pub_id: search.pub_id,
						}),
						db.saved_search()
							.delete(saved_search::id::equals(search_id)),
					)
					.await?;

					invalidate_query!(library, "search.saved.list");
					// disabled as it's messing with pre-delete navigation
					// invalidate_query!(library, "search.saved.get");

					Ok(())
				})
		})
}
