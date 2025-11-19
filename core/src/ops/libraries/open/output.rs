//! Output type for library open action

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryOpenOutput {
	/// ID of the opened library
	pub library_id: Uuid,

	/// Name of the opened library
	pub name: String,

	/// Path where the library is located
	pub path: PathBuf,
}

impl LibraryOpenOutput {
	pub fn new(library_id: Uuid, name: String, path: PathBuf) -> Self {
		Self {
			library_id,
			name,
			path,
		}
	}
}

