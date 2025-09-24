//! Library information query implementation

use super::output::LibraryInfoOutput;
use crate::{context::CoreContext, cqrs::LibraryQuery};
use anyhow::Result;
use std::sync::Arc;
use uuid::Uuid;

/// Query to get detailed information about a specific library
#[derive(Debug, Clone)]
pub struct LibraryInfoQuery;

impl LibraryQuery for LibraryInfoQuery {
	type Input = ();
	type Output = LibraryInfoOutput;

	fn from_input(input: Self::Input) -> Result<Self> {
		Ok(Self)
	}

	async fn execute(self, context: Arc<CoreContext>, library_id: Uuid) -> Result<Self::Output> {
		// Get the specific library from the library manager
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;

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

crate::register_library_query!(LibraryInfoQuery, "libraries.info");
