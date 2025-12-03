//! Input type for library open action

use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryOpenInput {
	/// Path to the library directory to open
	pub path: PathBuf,
}
