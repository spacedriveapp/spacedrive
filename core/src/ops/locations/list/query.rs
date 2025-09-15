use super::output::{LocationInfo, LocationsListOutput};
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use sea_orm::EntityTrait;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocationsListQuery {}

impl Query for LocationsListQuery {
	type Output = LocationsListOutput;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		// Get current library ID from session
		let session_state = context.session.get().await;
		let library_id = session_state
			.current_library_id
			.ok_or_else(|| anyhow::anyhow!("No active library selected"))?;

		// Fetch library and query locations table
		let library = context
			.libraries()
			.await
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found"))?;
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

crate::register_query!(LocationsListQuery, "locations.list");
