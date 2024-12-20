use sd_prisma::prisma;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Range<T> {
	From(T),
	To(T),
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

// #[derive(Deserialize, Type, Debug, Clone)]
// #[serde(untagged)]
// pub enum MaybeNot<T> {
// 	None(T),
// 	Not { not: T },
// }

// impl<T> MaybeNot<T> {
// 	pub fn into_prisma<R: From<prisma_client_rust::Operator<R>>>(self, param: fn(T) -> R) -> R {
// 		match self {
// 			Self::None(v) => param(v),
// 			Self::Not { not } => prisma_client_rust::not![param(not)],
// 		}
// 	}
// }

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

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
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

	pub fn into_param<TParam>(
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

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TextMatch {
	Contains(String),
	StartsWith(String),
	EndsWith(String),
	Equals(String),
}

impl TextMatch {
	pub fn is_empty(&self) -> bool {
		match self {
			Self::Contains(v) => v.is_empty(),
			Self::StartsWith(v) => v.is_empty(),
			Self::EndsWith(v) => v.is_empty(),
			Self::Equals(v) => v.is_empty(),
		}
	}

	// 3. Update the to_param method of TextMatch
	pub fn into_param<TParam>(
		self,
		contains_fn: fn(String) -> TParam,
		starts_with_fn: fn(String) -> TParam,
		ends_with_fn: fn(String) -> TParam,
		equals_fn: fn(String) -> TParam,
	) -> Option<TParam> {
		self.is_empty()
			.then_some(None)
			.unwrap_or_else(|| match self {
				Self::Contains(v) => Some(contains_fn(v)),
				Self::StartsWith(v) => Some(starts_with_fn(v)),
				Self::EndsWith(v) => Some(ends_with_fn(v)),
				Self::Equals(v) => Some(equals_fn(v)),
			})
	}
}
