//! Library listing query implementation

use super::output::LibraryInfo;
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Query to list all available libraries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListLibrariesQuery {
	/// Whether to include detailed statistics for each library
	pub include_stats: bool,
}

impl ListLibrariesQuery {
	/// Create a new query
	pub fn new(include_stats: bool) -> Self {
		Self { include_stats }
	}

	/// Create a query that includes statistics
	pub fn with_stats() -> Self {
		Self {
			include_stats: true,
		}
	}

	/// Create a basic query without statistics
	pub fn basic() -> Self {
		Self {
			include_stats: false,
		}
	}
}

impl Query for ListLibrariesQuery {
	type Output = Vec<LibraryInfo>;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		// Get all open libraries from the library manager
		let libraries = context.library_manager.list().await;
		let mut result = Vec::new();

		for library in libraries {
			// Get basic library information
			let id = library.id();
			let name = library.name().await;
			let path = library.path().to_path_buf();

			// Get statistics if requested
			let stats = if self.include_stats {
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
