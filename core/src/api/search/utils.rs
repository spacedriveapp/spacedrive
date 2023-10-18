use sd_prisma::prisma;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Deserialize, Default, Type, Debug)]
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

#[derive(Deserialize, Type, Debug)]
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
