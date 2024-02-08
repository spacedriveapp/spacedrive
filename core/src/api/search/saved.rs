use crate::{api::utils::library, invalidate_query, library::Library};

use sd_prisma::{prisma::saved_search, prisma_sync};
use sd_sync::OperationFactory;
use sd_utils::chain_optional_iter;

use chrono::{DateTime, FixedOffset, Utc};
use rspc::alpha::AlphaRouter;
use serde::{de::IgnoredAny, Deserialize, Serialize};
use serde_json::json;
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
					let Library { db, sync, .. } = library.as_ref();
					let pub_id = Uuid::new_v4().as_bytes().to_vec();
					let date_created: DateTime<FixedOffset> = Utc::now().into();

					let (sync_params, db_params): (Vec<_>, Vec<_>) = chain_optional_iter(
						[
							(
								(saved_search::date_created::NAME, json!(date_created)),
								saved_search::date_created::set(Some(date_created)),
							),
							(
								(saved_search::name::NAME, json!(&args.name)),
								saved_search::name::set(Some(args.name)),
							),
						],
						[
							args.filters
								.and_then(|s| {
									// https://github.com/serde-rs/json/issues/579
									// https://docs.rs/serde/latest/serde/de/struct.IgnoredAny.html

									if let Err(e) = serde_json::from_str::<IgnoredAny>(&s) {
										error!("failed to parse filters: {e:#?}");
										None
									} else {
										Some(s)
									}
								})
								.map(|v| {
									(
										(saved_search::filters::NAME, json!(&v)),
										saved_search::filters::set(Some(v)),
									)
								}),
							args.search.map(|v| {
								(
									(saved_search::search::NAME, json!(&v)),
									saved_search::search::set(Some(v)),
								)
							}),
							args.description.map(|v| {
								(
									(saved_search::description::NAME, json!(&v)),
									saved_search::description::set(Some(v)),
								)
							}),
							args.icon.map(|v| {
								(
									(saved_search::icon::NAME, json!(&v)),
									saved_search::icon::set(Some(v)),
								)
							}),
						],
					)
					.into_iter()
					.unzip();

					sync.write_ops(
						db,
						(
							sync.shared_create(
								prisma_sync::saved_search::SyncId {
									pub_id: pub_id.clone(),
								},
								sync_params,
							),
							db.saved_search().create(pub_id, db_params),
						),
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
					let updated_at = Utc::now().into();

					let search = db
						.saved_search()
						.find_unique(saved_search::id::equals(id))
						.select(saved_search::select!({ pub_id }))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(rspc::ErrorCode::NotFound, "search not found".into())
						})?;

					let (sync_params, db_params): (Vec<_>, Vec<_>) = chain_optional_iter(
						[(
							(saved_search::date_modified::NAME, json!(updated_at)),
							saved_search::date_modified::set(Some(updated_at)),
						)],
						[
							args.name.map(|v| {
								(
									(saved_search::name::NAME, json!(&v)),
									saved_search::name::set(v),
								)
							}),
							args.description.map(|v| {
								(
									(saved_search::name::NAME, json!(&v)),
									saved_search::name::set(v),
								)
							}),
							args.icon.map(|v| {
								(
									(saved_search::icon::NAME, json!(&v)),
									saved_search::icon::set(v),
								)
							}),
							args.search.map(|v| {
								(
									(saved_search::search::NAME, json!(&v)),
									saved_search::search::set(v),
								)
							}),
							args.filters.map(|v| {
								(
									(saved_search::filters::NAME, json!(&v)),
									saved_search::filters::set(v),
								)
							}),
						],
					)
					.into_iter()
					.map(|((k, v), p)| {
						(
							sync.shared_update(
								prisma_sync::saved_search::SyncId {
									pub_id: search.pub_id.clone(),
								},
								k,
								v,
							),
							p,
						)
					})
					.unzip();

					sync.write_ops(
						&db,
						(
							sync_params,
							db.saved_search()
								.update_unchecked(saved_search::id::equals(id), db_params),
						),
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
						&db,
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
