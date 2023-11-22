use sd_prisma::prisma::{self, media_data};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::utils::*;

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum MediaDataOrder {
	EpochTime(SortOrder),
}

impl MediaDataOrder {
	pub fn get_sort_order(&self) -> prisma::SortOrder {
		(*match self {
			Self::EpochTime(v) => v,
		})
		.into()
	}

	pub fn into_param(self) -> media_data::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use media_data::*;
		match self {
			Self::EpochTime(_) => epoch_time::order(dir),
		}
	}
}
