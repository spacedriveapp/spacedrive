use chrono::{DateTime, FixedOffset};
use prisma_client_rust::{or, OrderByQuery, PaginatedQuery, WhereQuery};
use sd_prisma::prisma::{self, object, tag_on_object};
use serde::{Deserialize, Serialize};
use specta::Type;

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

impl ObjectCursor {
	fn apply(self, query: &mut object::FindManyQuery, id: i32) {
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

		match self {
			Self::None => {
				query.add_where(prisma::object::id::gt(id));
			}
			Self::Kind(item) => arm!(kind, item),
			Self::DateAccessed(item) => arm!(date_accessed, item),
		}
	}
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum ObjectOrder {
	DateAccessed(SortOrder),
	Kind(SortOrder),
	MediaData(Box<MediaDataOrder>),
}

impl ObjectOrder {
	pub fn get_sort_order(&self) -> prisma::SortOrder {
		(*match self {
			Self::DateAccessed(v) => v,
			Self::Kind(v) => v,
			Self::MediaData(v) => return v.get_sort_order(),
		})
		.into()
	}

	pub fn into_param(self) -> object::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use object::*;

		match self {
			Self::DateAccessed(_) => date_accessed::order(dir),
			Self::Kind(_) => kind::order(dir),
			Self::MediaData(v) => media_data::order(vec![v.into_param()]),
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

#[derive(Deserialize, Type, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ObjectFilterArgs {
	#[specta(optional)]
	favorite: Option<bool>,
	#[serde(default)]
	hidden: ObjectHiddenFilter,
	#[specta(optional)]
	date_accessed: Option<MaybeNot<Option<chrono::DateTime<FixedOffset>>>>,
	#[serde(default)]
	#[specta(optional)]
	kind: Option<InOrNotIn<i32>>,
	#[serde(default)]
	#[specta(optional)]
	tags: Option<InOrNotIn<i32>>,
	#[serde(default)]
	#[specta(optional)]
	category: Option<InOrNotIn<Category>>,
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
				self.kind
					.and_then(|v| v.to_param(kind::in_vec, kind::not_in_vec)),
				self.tags.and_then(|v| {
					v.to_param(
						|v| tags::some(vec![tag_on_object::tag_id::in_vec(v)]),
						|v| tags::none(vec![tag_on_object::tag_id::in_vec(v)]),
					)
				}),
				self.category.and_then(|v| {
					v.to_param(
						|v| {
							prisma_client_rust::operator::and(
								v.into_iter().map(Category::to_where_param).collect(),
							)
						},
						|v| {
							prisma_client_rust::operator::not(
								v.into_iter().map(Category::to_where_param).collect(),
							)
						},
					)
				}),
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
				cursor.apply(query, id);

				query.add_order_by(prisma::object::pub_id::order(prisma::SortOrder::Asc))
			}
		}
	}
}
