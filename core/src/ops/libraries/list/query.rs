//! Library listing query implementation

use super::output::LibraryInfo;
use crate::{context::CoreContext, cqrs::CoreQuery};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListLibrariesInput {
	/// Whether to include detailed statistics for each library
	pub include_stats: bool,
}

/// Query to list all available libraries
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListLibrariesQuery {
	pub input: ListLibrariesInput,
}

impl ListLibrariesQuery {
	/// Create a basic library list query without statistics
	pub fn basic() -> Self {
		Self {
			input: ListLibrariesInput {
				include_stats: false,
			},
		}
	}
}

impl CoreQuery for ListLibrariesQuery {
	type Input = ListLibrariesInput;
	type Output = Vec<LibraryInfo>;

	fn from_input(input: Self::Input) -> Result<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> Result<Self::Output> {
		// Get all open libraries from the library manager
		let libraries = context.libraries().await.list().await;
		let mut result = Vec::new();

		for library in libraries {
			// Get basic library information
			let id = library.id();
			let name = library.name().await;
			let path = library.path().to_path_buf();

			// Get statistics if requested
			let stats = if self.input.include_stats {
				// Get the library config which contains statistics
				let config = library.config().await;
				Some(config.statistics)
			} else {
				None
			};

			result.push(LibraryInfo::new(id, name, path, stats));
		}

		Ok(result)
	}
}

crate::register_core_query!(ListLibrariesQuery, "libraries.list");
