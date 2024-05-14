use sd_prisma::prisma::{self, exif_data};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::utils::*;

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum ExifDataOrder {
	EpochTime(SortOrder),
}

impl ExifDataOrder {
	pub fn get_sort_order(&self) -> prisma::SortOrder {
		(*match self {
			Self::EpochTime(v) => v,
		})
		.into()
	}

	pub fn into_param(self) -> exif_data::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use exif_data::*;
		match self {
			Self::EpochTime(_) => epoch_time::order(dir),
		}
	}
}
