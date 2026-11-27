use super::output::LayoutGetOutput;
use crate::domain::Layout;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutGetQueryInput {
	pub layout_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutGetQuery {
	layout_id: Uuid,
}

impl LibraryQuery for LayoutGetQuery {
	type Input = LayoutGetQueryInput;
	type Output = LayoutGetOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self {
			layout_id: input.layout_id,
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

		let layout_model = crate::infra::db::entities::layout::Entity::find()
			.filter(crate::infra::db::entities::layout::Column::Uuid.eq(self.layout_id))
			.one(db)
			.await?
			.ok_or_else(|| QueryError::Internal(format!("Layout {} not found", self.layout_id)))?;

		let layout = Layout {
			id: layout_model.uuid,
			name: layout_model.name,
			icon: layout_model.icon,
			color: layout_model.color,
			order: layout_model.order,
			created_at: layout_model.created_at.into(),
			updated_at: layout_model.updated_at.into(),
		};

		Ok(LayoutGetOutput { layout })
	}
}

crate::register_library_query!(LayoutGetQuery, "layouts.get");
