//! Library information query implementation

use super::output::LibraryInfoOutput;
use crate::{context::CoreContext, cqrs::LibraryQuery};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

/// Input for library info query
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LibraryInfoQueryInput;

/// Query to get detailed information about a specific library
#[derive(Debug, Clone)]
pub struct LibraryInfoQuery;

impl LibraryQuery for LibraryInfoQuery {
	type Input = LibraryInfoQueryInput;
	type Output = LibraryInfoOutput;

	fn from_input(input: Self::Input) -> Result<Self> {
		Ok(Self)
	}

	async fn execute(self, context: Arc<CoreContext>, session: crate::infra::api::SessionContext) -> Result<Self::Output> {
		// Get the specific library from the library manager
		let library_id = session.current_library_id.ok_or_else(|| anyhow::anyhow!("No library in session"))?;
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
