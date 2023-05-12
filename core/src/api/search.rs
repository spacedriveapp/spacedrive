use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR};

use chrono::{DateTime, Utc};
use prisma_client_rust::{operator::or, Direction};
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::{
	api::{
		locations::{file_path_with_object, ExplorerItem},
		utils::library,
	},
	library::Library,
	location::{find_location, LocationError},
	prisma::*,
	util::db::chain_optional_iter,
};

use super::{Ctx, R};

#[derive(Serialize, Type, Debug)]
struct SearchData<T> {
	cursor: Option<Vec<u8>>,
	items: Vec<T>,
}

pub fn mount() -> AlphaRouter<Ctx> {
	R.router().procedure("paths", {
		#[derive(Deserialize, Type, Debug, Clone, Copy)]
		#[serde(rename_all = "camelCase")]
		#[specta(inline)]
		enum Ordering {
			Name(bool),
		}
		impl Ordering {
			fn get_direction(&self) -> Direction {
				match self {
					Self::Name(v) => v,
				}
				.then_some(Direction::Asc)
				.unwrap_or(Direction::Desc)
			}
			fn to_param(self) -> file_path::OrderByParam {
				let dir = self.get_direction();
				use file_path::*;
				match self {
					Self::Name(_) => name::order(dir),
				}
			}
		}
		#[derive(Deserialize, Type, Debug)]
		#[serde(rename_all = "camelCase")]
		#[specta(inline)]
		struct Args {
			#[specta(optional)]
			location_id: Option<i32>,
			#[specta(optional)]
			after_file_id: Option<Uuid>,
			#[specta(optional)]
			take: Option<i32>,
			#[specta(optional)]
			order: Option<Ordering>,
			#[serde(default)]
			#[specta(optional)]
			search: String,
			#[specta(optional)]
			extension: Option<String>,
			#[serde(default)]
			#[specta(optional)]
			kind: Vec<i32>,
			#[serde(default)]
			#[specta(optional)]
			tags: Vec<i32>,
			#[specta(optional)]
			created_at_from: Option<DateTime<Utc>>,
			#[specta(optional)]
			created_at_to: Option<DateTime<Utc>>,
			#[specta(optional)]
			path: Option<String>,
			#[specta(optional)]
			cursor: Option<Vec<u8>>,
		}

		R.with2(library())
			.query(|(_, library), args: Args| async move {
				let Library { db, .. } = &library;

				let location = if let Some(location_id) = args.location_id {
					Some(
						find_location(&library, location_id)
							.exec()
							.await?
							.ok_or(LocationError::IdNotFound(location_id))?,
					)
				} else {
					None
				};

				let directory_id = if let Some(mut path) = args.path.clone() {
					if !path.ends_with(MAIN_SEPARATOR) {
						path += MAIN_SEPARATOR_STR;
					}

					Some(
						db.file_path()
							.find_first(chain_optional_iter(
								[
									file_path::materialized_path::equals(path),
									file_path::is_dir::equals(true),
								],
								[location.map(|l| file_path::location_id::equals(l.id))],
							))
							.select(file_path::select!({ pub_id }))
							.exec()
							.await?
							.ok_or_else(|| {
								rspc::Error::new(ErrorCode::NotFound, "Directory not found".into())
							})?
							.pub_id,
					)
				} else {
					None
				};

				let object_params = chain_optional_iter(
					[],
					[
						(!args.kind.is_empty()).then(|| object::kind::in_vec(args.kind)),
						(!args.tags.is_empty()).then(|| {
							let tags = args.tags.into_iter().map(tag::id::equals).collect();
							let tags_on_object = tag_on_object::tag::is(vec![or(tags)]);

							object::tags::some(vec![tags_on_object])
						}),
					],
				);

				let params = chain_optional_iter(
					args.search
						.split(' ')
						.map(str::to_string)
						.map(file_path::materialized_path::contains),
					[
						args.location_id.map(file_path::location_id::equals),
						args.extension.map(file_path::extension::equals),
						args.created_at_from
							.map(|v| file_path::date_created::gte(v.into())),
						args.created_at_to
							.map(|v| file_path::date_created::lte(v.into())),
						args.path.map(file_path::materialized_path::starts_with),
						directory_id.map(Some).map(file_path::parent_id::equals),
						(!object_params.is_empty()).then(|| file_path::object::is(object_params)),
					],
				);

				let take = args.take.unwrap_or(100);

				let mut query = db.file_path().find_many(params).take(take as i64 + 1);

				if let Some(file_id) = args.after_file_id {
					query = query.cursor(file_path::pub_id::equals(file_id.as_bytes().to_vec()))
				}

				if let Some(order) = args.order {
					query = query.order_by(order.to_param());
				}

				if let Some(cursor) = args.cursor {
					query = query.cursor(file_path::pub_id::equals(cursor));
				}

				let (file_paths, cursor) = {
					let mut paths = query
						.include(file_path_with_object::include())
						.exec()
						.await?;

					let cursor = (paths.len() as i32 > take)
						.then(|| paths.pop())
						.flatten()
						.map(|r| r.pub_id);

					(paths, cursor)
				};

				let mut items = Vec::with_capacity(file_paths.len());

				for file_path in file_paths {
					let has_thumbnail = if let Some(cas_id) = &file_path.cas_id {
						library
							.thumbnail_exists(cas_id)
							.await
							.map_err(LocationError::IOError)?
					} else {
						false
					};

					items.push(ExplorerItem::Path {
						has_thumbnail,
						item: file_path,
					})
				}

				Ok(SearchData { items, cursor })
			})
	})
}
