use sd_prisma::prisma;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Deserialize, Default, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OptionalRange<T> {
	pub from: Option<T>,
	pub to: Option<T>,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "PascalCase")]
pub enum SortOrder {
	Asc,
	Desc,
}

impl From<SortOrder> for prisma::SortOrder {
	fn from(value: SortOrder) -> prisma::SortOrder {
		match value {
			SortOrder::Asc => prisma::SortOrder::Asc,
			SortOrder::Desc => prisma::SortOrder::Desc,
		}
	}
}

#[derive(Deserialize, Type, Debug, Clone)]
#[serde(untagged)]
pub enum MaybeNot<T> {
	None(T),
	Not { not: T },
}

impl<T> MaybeNot<T> {
	pub fn into_prisma<R: From<prisma_client_rust::Operator<R>>>(self, param: fn(T) -> R) -> R {
		match self {
			Self::None(v) => param(v),
			Self::Not { not } => prisma_client_rust::not![param(not)],
		}
	}
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CursorOrderItem<T> {
	pub order: SortOrder,
	pub data: T,
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum OrderAndPagination<TId, TOrder, TCursor> {
	OrderOnly(TOrder),
	Offset { offset: i32, order: Option<TOrder> },
	Cursor { id: TId, cursor: TCursor },
}

#[derive(Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum InOrNotIn<T> {
	In(Vec<T>),
	NotIn(Vec<T>),
}

impl<T> InOrNotIn<T> {
	pub fn is_empty(&self) -> bool {
		match self {
			Self::In(v) => v.is_empty(),
			Self::NotIn(v) => v.is_empty(),
		}
	}

	pub fn to_param<TParam>(
		self,
		in_fn: fn(Vec<T>) -> TParam,
		not_in_fn: fn(Vec<T>) -> TParam,
	) -> Option<TParam> {
		self.is_empty()
			.then_some(None)
			.unwrap_or_else(|| match self {
				Self::In(v) => Some(in_fn(v)),
				Self::NotIn(v) => Some(not_in_fn(v)),
			})
	}
}
