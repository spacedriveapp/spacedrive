use super::output::SpaceGetOutput;
use crate::domain::Space;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceGetQueryInput {
	pub space_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceGetQuery {
	space_id: Uuid,
}

impl LibraryQuery for SpaceGetQuery {
	type Input = SpaceGetQueryInput;
	type Output = SpaceGetOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			space_id: input.space_id,
		})
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

		let db = library.db().conn();

		let space_model = crate::infra::db::entities::space::Entity::find()
			.filter(crate::infra::db::entities::space::Column::Uuid.eq(self.space_id))
			.one(db)
			.await?
			.ok_or_else(|| QueryError::Internal(format!("Space {} not found", self.space_id)))?;

		let space = Space {
			id: space_model.uuid,
			name: space_model.name,
			icon: space_model.icon,
			color: space_model.color,
			order: space_model.order,
			created_at: space_model.created_at.into(),
			updated_at: space_model.updated_at.into(),
		};

		Ok(SpaceGetOutput { space })
	}
}

crate::register_library_query!(SpaceGetQuery, "spaces.get");
