//! List available adapters query

use crate::{
	context::CoreContext,
	infra::query::{LibraryQuery, QueryError, QueryResult},
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListAdaptersInput {}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AdapterInfo {
	pub id: String,
	pub name: String,
	pub description: String,
	pub version: String,
	pub author: String,
	pub data_type: String,
	pub icon_svg: Option<String>,
	pub update_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAdaptersQuery {
	pub input: ListAdaptersInput,
}

impl LibraryQuery for ListAdaptersQuery {
	type Input = ListAdaptersInput;
	type Output = Vec<AdapterInfo>;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		if library.source_manager().is_none() {
			library
				.init_source_manager()
				.await
				.map_err(|e| QueryError::Internal(format!("Failed to init source manager: {e}")))?;
		}

		let source_manager = library
			.source_manager()
			.ok_or_else(|| QueryError::Internal("Source manager not available".to_string()))?;

		let adapters = source_manager.list_adapters();

		Ok(adapters
			.into_iter()
			.map(|a| AdapterInfo {
				id: a.id,
				name: a.name,
				description: a.description,
				version: a.version,
				author: a.author,
				data_type: a.data_type,
				icon_svg: a.icon_svg,
				update_available: a.update_available,
			})
			.collect())
	}
}

crate::register_library_query!(ListAdaptersQuery, "adapters.list");
