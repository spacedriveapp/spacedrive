use super::output::LayoutsListOutput;
use crate::domain::Layout;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{EntityTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutsListQueryInput;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutsListQuery;

impl LibraryQuery for LayoutsListQuery {
	type Input = LayoutsListQueryInput;
	type Output = LayoutsListOutput;

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

		let db = library.db().conn();

		let layout_models = crate::infra::db::entities::layout::Entity::find()
			.order_by_asc(crate::infra::db::entities::layout::Column::Order)
			.all(db)
			.await?;

		let layouts = layout_models
			.into_iter()
			.map(|model| Layout {
				id: model.uuid,
				name: model.name,
				icon: model.icon,
				color: model.color,
				order: model.order,
				created_at: model.created_at.into(),
				updated_at: model.updated_at.into(),
			})
			.collect();

		Ok(LayoutsListOutput { layouts })
	}
}

crate::register_library_query!(LayoutsListQuery, "layouts.list");
