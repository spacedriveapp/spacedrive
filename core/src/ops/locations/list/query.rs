use super::output::{LocationInfo, LocationsListOutput};
use crate::{context::CoreContext, cqrs::Query};
use anyhow::Result;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocationsListQuery {
	pub library_id: uuid::Uuid,
}

impl Query for LocationsListQuery {
	type Output = LocationsListOutput;

	async fn execute(self, context: Arc<CoreContext>) -> Result<Self::Output> {
		// Fetch library and query locations table
		let library = context.library_manager.get_library(self.library_id).await.ok_or_else(|| anyhow::anyhow!("Library not found"))?;
		let db = library.db().conn();
		let rows = crate::infra::db::entities::location::Entity::find()
			.filter(crate::infra::db::entities::location::Column::LibraryId.eq(self.library_id))
			.all(db)
			.await?;
		let mut out = Vec::new();
		for r in rows {
			out.push(LocationInfo { id: r.uuid, path: std::path::PathBuf::from(r.path), name: r.name });
		}
		Ok(LocationsListOutput { locations: out })
	}
}

crate::register_query!(LocationsListQuery, "locations.list");

