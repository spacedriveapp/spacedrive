// use crate::library::Category;

use sd_prisma::prisma::{self, label_on_object, object, tag_on_object};

use chrono::{DateTime, FixedOffset};
use prisma_client_rust::{not, or, OrderByQuery, PaginatedQuery, WhereQuery};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{
	exif_data::*,
	utils::{self, *},
};

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
						SortOrder::Asc => object::$field::gt(data),
						SortOrder::Desc => object::$field::lt(data),
					},
					prisma_client_rust::and![
						object::$field::equals(Some(item.data)),
						match item.order {
							SortOrder::Asc => object::id::gt(id),
							SortOrder::Desc => object::id::lt(id),
						}
					]
				]);

				query.add_order_by(object::$field::order(item.order.into()));
			}};
		}

		match self {
			Self::None => {
				query.add_where(object::id::gt(id));
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
	MediaData(Box<ExifDataOrder>),
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
			Self::MediaData(v) => exif_data::order(vec![v.into_param()]),
		}
	}
}

#[derive(Serialize, Deserialize, Type, Debug, Default, Clone, Copy)]
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

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ObjectFilterArgs {
	Favorite(bool),
	Hidden(ObjectHiddenFilter),
	Kind(InOrNotIn<i32>),
	Tags(InOrNotIn<i32>),
	Labels(InOrNotIn<i32>),
	DateAccessed(Range<chrono::DateTime<FixedOffset>>),
}

impl ObjectFilterArgs {
	pub fn into_params(self) -> Vec<object::WhereParam> {
		use object::*;

		match self {
			Self::Favorite(v) => vec![favorite::equals(Some(v))],
			Self::Hidden(v) => v.to_param().map(|v| vec![v]).unwrap_or_default(),
			Self::Tags(v) => v
				.into_param(
					|v| tags::some(vec![tag_on_object::tag_id::in_vec(v)]),
					|v| tags::none(vec![tag_on_object::tag_id::in_vec(v)]),
				)
				.map(|v| vec![v])
				.unwrap_or_default(),
			Self::Labels(v) => v
				.into_param(
					|v| labels::some(vec![label_on_object::label_id::in_vec(v)]),
					|v| labels::none(vec![label_on_object::label_id::in_vec(v)]),
				)
				.map(|v| vec![v])
				.unwrap_or_default(),
			Self::Kind(v) => v
				.into_param(kind::in_vec, kind::not_in_vec)
				.map(|v| vec![v])
				.unwrap_or_default(),
			Self::DateAccessed(v) => {
				vec![
					not![date_accessed::equals(None)],
					match v {
						Range::From(v) => date_accessed::gte(v),
						Range::To(v) => date_accessed::lte(v),
					},
				]
			}
		}
	}
}

pub type OrderAndPagination =
	utils::OrderAndPagination<object::id::Type, ObjectOrder, ObjectCursor>;

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

				query.add_order_by(object::pub_id::order(prisma::SortOrder::Asc))
			}
		}
	}
}
