//! Search filter utilities

use super::input::*;
use crate::domain::ContentKind;
use crate::filetype::FileTypeRegistry;
use sea_orm::{ColumnTrait, Condition};

/// Filter builder for search queries
pub struct FilterBuilder {
	condition: Condition,
}

impl FilterBuilder {
	pub fn new() -> Self {
		Self {
			condition: Condition::all(),
		}
	}

	pub fn build(self) -> Condition {
		self.condition
	}

	/// Apply file type filter
	pub fn file_types(mut self, file_types: &Option<Vec<String>>) -> Self {
		if let Some(types) = file_types {
			if !types.is_empty() {
				let mut file_type_condition = Condition::any();
				for file_type in types {
					file_type_condition = file_type_condition
						.add(crate::infra::db::entities::entry::Column::Extension.eq(file_type));
				}
				self.condition = self.condition.add(file_type_condition);
			}
		}
		self
	}

	/// Apply date range filter
	pub fn date_range(mut self, date_range: &Option<DateRangeFilter>) -> Self {
		if let Some(range) = date_range {
			let date_column = match range.field {
				DateField::CreatedAt => crate::infra::db::entities::entry::Column::CreatedAt,
				DateField::ModifiedAt => crate::infra::db::entities::entry::Column::ModifiedAt,
				DateField::AccessedAt => crate::infra::db::entities::entry::Column::AccessedAt,
				DateField::IndexedAt => crate::infra::db::entities::entry::Column::IndexedAt,
			};

			if let Some(start) = range.start {
				self.condition = self.condition.add(date_column.gte(start));
			}
			if let Some(end) = range.end {
				self.condition = self.condition.add(date_column.lte(end));
			}
		}
		self
	}

	/// Apply size range filter
	pub fn size_range(mut self, size_range: &Option<SizeRangeFilter>) -> Self {
		if let Some(range) = size_range {
			if let Some(min) = range.min {
				self.condition = self
					.condition
					.add(crate::infra::db::entities::entry::Column::Size.gte(min as i64));
			}
			if let Some(max) = range.max {
				self.condition = self
					.condition
					.add(crate::infra::db::entities::entry::Column::Size.lte(max as i64));
			}
		}
		self
	}

	/// Apply location filter
	pub fn locations(mut self, locations: &Option<Vec<uuid::Uuid>>) -> Self {
		if let Some(locs) = locations {
			if !locs.is_empty() {
				// TODO: Add location filtering when location_id is available in entry table
				// let mut location_condition = Condition::any();
				// for location_id in locs {
				//     location_condition = location_condition.add(
				//         crate::infra::db::entities::entry::Column::LocationId.eq(*location_id)
				//     );
				// }
				// self.condition = self.condition.add(location_condition);
			}
		}
		self
	}

	/// Apply content type filter using the file type registry
	pub fn content_types(
		mut self,
		content_types: &Option<Vec<ContentKind>>,
		registry: &FileTypeRegistry,
	) -> Self {
		if let Some(types) = content_types {
			if !types.is_empty() {
				let mut content_condition = Condition::any();
				for content_type in types {
					let extensions = registry.get_extensions_for_category(*content_type);
					for extension in extensions {
						content_condition = content_condition.add(
							crate::infra::db::entities::entry::Column::Extension.eq(extension),
						);
					}
				}
				self.condition = self.condition.add(content_condition);
			}
		}
		self
	}

	/// Apply hidden files filter
	pub fn include_hidden(mut self, include_hidden: &Option<bool>) -> Self {
		if let Some(include) = include_hidden {
			if !include {
				// TODO: Add hidden field to entry table
				// self.condition = self.condition.add(
				//     crate::infra::db::entities::entry::Column::Hidden.eq(false)
				// );
			}
		}
		self
	}
}

// Removed hardcoded extension mapping - now using FileTypeRegistry

impl Default for FilterBuilder {
	fn default() -> Self {
		Self::new()
	}
}
