//! Input types for library deletion operations

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Input for deleting a library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDeleteInput {
	/// ID of the library to delete
	pub library_id: Uuid,

	/// Whether to also delete the library's data directory
	pub delete_data: bool,
}

impl LibraryDeleteInput {
	/// Create a new library deletion input
	pub fn new(library_id: Uuid) -> Self {
		Self {
			library_id,
			delete_data: true,
		}
	}

	/// Set whether to delete data
	pub fn with_delete_data(mut self, delete_data: bool) -> Self {
		self.delete_data = delete_data;
		self
	}

	/// Validate the input
	pub fn validate(&self) -> Result<(), Vec<String>> {
		let mut errors = Vec::new();

		if self.library_id.is_nil() {
			errors.push("Library ID cannot be nil".to_string());
		}

		if errors.is_empty() {
			Ok(())
		} else {
			Err(errors)
		}
	}
}
