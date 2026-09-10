use super::output::{SuggestedLocation, SuggestedLocationsOutput};
use crate::domain::addressing::SdPath;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SuggestedLocationsQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SuggestedLocationsQuery;

impl LibraryQuery for SuggestedLocationsQuery {
	type Input = SuggestedLocationsQueryInput;
	type Output = SuggestedLocationsOutput;

	fn from_input(_input: Self::Input) -> QueryResult<Self> {
		Ok(Self {})
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library_id = session
			.current_library_id
			.ok_or_else(|| QueryError::Internal("No library selected".to_string()))?;

		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;

		// What is already pinned, so a suggestion is only ever something new.
		let pinned: std::collections::HashSet<PathBuf> = crate::location::list(&library, &context)
			.await
			.map_err(|e| QueryError::Internal(e.to_string()))?
			.into_iter()
			.filter_map(|location| location.path().map(std::path::Path::to_path_buf))
			.collect();

		let device_slug = crate::device::get_current_device_slug();

		let result = crate::location::default_paths()
			.into_iter()
			.filter(|(_, path)| !pinned.contains(path))
			.map(|(name, path)| SuggestedLocation {
				name,
				sd_path: SdPath::Physical {
					device_slug: device_slug.clone(),
					path: path.clone(),
				},
				path,
			})
			.collect();

		Ok(SuggestedLocationsOutput { locations: result })
	}
}

crate::register_library_query!(SuggestedLocationsQuery, "locations.suggested");
