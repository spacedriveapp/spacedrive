use super::output::{LocationInfo, LocationsListOutput};
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LocationsListQuery;

impl LibraryQuery for LocationsListQuery {
	type Input = LocationsListQueryInput;
	type Output = LocationsListOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
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
		// Fetch library and query locations table
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| QueryError::Internal("Library not found".to_string()))?;
		let db = library.db().conn();
		let rows = crate::infra::db::entities::location::Entity::find()
			.all(db)
			.await?;
		let mut out = Vec::new();
		for r in rows {
			out.push(LocationInfo {
				id: r.uuid,
				path: std::path::PathBuf::from(r.name.clone().unwrap_or_default()),
				name: r.name.clone(),
			});
		}
		Ok(LocationsListOutput { locations: out })
	}
}

crate::register_library_query!(LocationsListQuery, "locations.list");
