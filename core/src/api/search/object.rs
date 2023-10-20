use std::collections::BTreeSet;

use chrono::{DateTime, FixedOffset};
use prisma_client_rust::{operator, or, OrderByQuery, PaginatedQuery, WhereQuery};

use serde::{Deserialize, Serialize};
use specta::Type;

use sd_prisma::prisma::{self, media_data, object, tag, tag_on_object};

use crate::library::Category;

use super::media_data::*;
use super::utils::{self, *};

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ObjectCursor {
	None,
	DateAccessed(CursorOrderItem<DateTime<FixedOffset>>),
	Kind(CursorOrderItem<i32>),
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum ObjectOrder {
	DateAccessed(SortOrder),
	Kind(SortOrder),
	DateImageTaken(SortOrder),
}

impl ObjectOrder {
	pub fn get_sort_order(&self) -> prisma::SortOrder {
		(*match self {
			Self::DateAccessed(v) => v,
			Self::Kind(v) => v,
			Self::DateImageTaken(v) => v,
		})
		.into()
	}

	pub fn media_data(
		&self,
		param: MediaDataSortParameter,
		dir: prisma::SortOrder,
	) -> object::OrderByWithRelationParam {
		let order = match param {
			MediaDataSortParameter::DateImageTaken => media_data::epoch_time::order(dir),
		};

		object::media_data::order(vec![order])
	}

	pub fn into_param(self) -> object::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use object::*;

		match self {
			Self::DateAccessed(_) => date_accessed::order(dir),
			Self::Kind(_) => kind::order(dir),
			Self::DateImageTaken(_) => self.media_data(MediaDataSortParameter::DateImageTaken, dir),
		}
	}
}

#[derive(Deserialize, Type, Debug, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum ObjectHiddenFilter {
	#[default]
	Exclude,
	Include,
}

impl ObjectHiddenFilter {
	pub fn to_param(self) -> Option<object::WhereParam> {
		match self {
			ObjectHiddenFilter::Exclude => Some(or![
				object::hidden::equals(None),
				object::hidden::not(Some(true))
			]),
			ObjectHiddenFilter::Include => None,
		}
	}
}

#[derive(Deserialize, Type, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ObjectFilterArgs {
	#[specta(optional)]
	favorite: Option<bool>,
	#[serde(default)]
	hidden: ObjectHiddenFilter,
	#[specta(optional)]
	date_accessed: Option<MaybeNot<Option<chrono::DateTime<FixedOffset>>>>,
	#[serde(default)]
	kind: BTreeSet<i32>,
	#[serde(default)]
	tags: Vec<i32>,
	#[specta(optional)]
	category: Option<Category>,
}

impl ObjectFilterArgs {
	pub fn into_params(self) -> Vec<object::WhereParam> {
		use object::*;

		sd_utils::chain_optional_iter(
			[],
			[
				self.hidden.to_param(),
				self.favorite.map(Some).map(favorite::equals),
				self.date_accessed
					.map(|date| date.into_prisma(date_accessed::equals)),
				(!self.kind.is_empty()).then(|| kind::in_vec(self.kind.into_iter().collect())),
				(!self.tags.is_empty()).then(|| {
					let tags = self.tags.into_iter().map(tag::id::equals).collect();
					let tags_on_object = tag_on_object::tag::is(vec![operator::or(tags)]);

					tags::some(vec![tags_on_object])
				}),
				self.category.map(Category::to_where_param),
			],
		)
	}
}

pub type OrderAndPagination =
	utils::OrderAndPagination<prisma::object::id::Type, ObjectOrder, ObjectCursor>;

impl OrderAndPagination {
	pub fn apply(self, query: &mut object::FindManyQuery) {
		match self {
			Self::OrderOnly(order) => {
				query.add_order_by(order.into_param());
			}
			Self::Offset { offset, order } => {
				query.set_skip(offset as i64);

				if let Some(order) = order {
					query.add_order_by(order.into_param())
				}
			}
			Self::Cursor { id, cursor } => {
				macro_rules! arm {
					($field:ident, $item:ident) => {{
						let item = $item;

						let data = item.data.clone();

						query.add_where(or![
							match item.order {
								SortOrder::Asc => prisma::object::$field::gt(data),
								SortOrder::Desc => prisma::object::$field::lt(data),
							},
							prisma_client_rust::and![
								prisma::object::$field::equals(Some(item.data)),
								match item.order {
									SortOrder::Asc => prisma::object::id::gt(id),
									SortOrder::Desc => prisma::object::id::lt(id),
								}
							]
						]);

						query.add_order_by(prisma::object::$field::order(item.order.into()));
					}};
				}

				match cursor {
					ObjectCursor::None => {
						query.add_where(prisma::object::id::gt(id));
					}
					ObjectCursor::Kind(item) => arm!(kind, item),
					ObjectCursor::DateAccessed(item) => arm!(date_accessed, item),
				}

				query.add_order_by(prisma::object::pub_id::order(prisma::SortOrder::Asc))
			}
		}
	}
}
