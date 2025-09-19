//! Library information query implementation

use super::output::LibraryInfoOutput;
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Query to get detailed information about a specific library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfoQuery {
	/// ID of the library to get information about
	pub library_id: Uuid,
}

impl LibraryInfoQuery {
	/// Create a new query
	pub fn new(library_id: Uuid) -> Self {
		Self { library_id }
	}
}

impl Query for LibraryInfoQuery {
	type Output = LibraryInfoOutput;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		// Get the specific library from the library manager
		let library = context
			.libraries()
			.await
			.get_library(self.library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found: {}", self.library_id))?;

		// Get library configuration which contains all the details
		let config = library.config().await;

		// Get library path
		let path = library.path().to_path_buf();

		Ok(LibraryInfoOutput {
			id: config.id,
			name: config.name,
			description: config.description,
			path,
			created_at: config.created_at,
			updated_at: config.updated_at,
			settings: config.settings,
			statistics: config.statistics,
		})
	}
}

crate::register_query!(LibraryInfoQuery, "libraries.info");
