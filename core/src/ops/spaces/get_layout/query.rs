use super::output::SpaceLayoutOutput;
use crate::domain::{
	GroupType, ItemType, Space, SpaceGroup, SpaceGroupWithItems, SpaceItem, SpaceLayout,
};
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceLayoutQueryInput {
	pub space_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceLayoutQuery {
	space_id: Uuid,
}

impl LibraryQuery for SpaceLayoutQuery {
	type Input = SpaceLayoutQueryInput;
	type Output = SpaceLayoutOutput;

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

		// Get space
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

		// Get space-level items (no group)
		let space_item_models = crate::infra::db::entities::space_item::Entity::find()
			.filter(crate::infra::db::entities::space_item::Column::SpaceId.eq(space_model.id))
			.filter(crate::infra::db::entities::space_item::Column::GroupId.is_null())
			.order_by_asc(crate::infra::db::entities::space_item::Column::Order)
			.all(db)
			.await?;

		let mut space_items = Vec::new();

		for item_model in space_item_models {
			let item_type: ItemType = serde_json::from_str(&item_model.item_type)
				.map_err(|e| QueryError::Internal(format!("Failed to parse item_type: {}", e)))?;

			space_items.push(SpaceItem {
				id: item_model.uuid,
				space_id: self.space_id,
				group_id: None,
				item_type,
				order: item_model.order,
				created_at: item_model.created_at.into(),
			});
		}

		// Get groups for this space
		let group_models = crate::infra::db::entities::space_group::Entity::find()
			.filter(crate::infra::db::entities::space_group::Column::SpaceId.eq(space_model.id))
			.order_by_asc(crate::infra::db::entities::space_group::Column::Order)
			.all(db)
			.await?;

		let mut groups = Vec::new();

		for group_model in group_models {
			// Parse group_type JSON
			let group_type: GroupType = serde_json::from_str(&group_model.group_type)
				.map_err(|e| QueryError::Internal(format!("Failed to parse group_type: {}", e)))?;

			let group = SpaceGroup {
				id: group_model.uuid,
				space_id: self.space_id,
				name: group_model.name,
				group_type,
				is_collapsed: group_model.is_collapsed,
				order: group_model.order,
				created_at: group_model.created_at.into(),
			};

			// Get items for this group
			let item_models = crate::infra::db::entities::space_item::Entity::find()
				.filter(
					crate::infra::db::entities::space_item::Column::GroupId
						.eq(Some(group_model.id)),
				)
				.order_by_asc(crate::infra::db::entities::space_item::Column::Order)
				.all(db)
				.await?;

			let mut items = Vec::new();

			for item_model in item_models {
				// Parse item_type JSON
				let item_type: ItemType =
					serde_json::from_str(&item_model.item_type).map_err(|e| {
						QueryError::Internal(format!("Failed to parse item_type: {}", e))
					})?;

				items.push(SpaceItem {
					id: item_model.uuid,
					space_id: self.space_id,
					group_id: Some(group_model.uuid),
					item_type,
					order: item_model.order,
					created_at: item_model.created_at.into(),
				});
			}

			groups.push(SpaceGroupWithItems { group, items });
		}

		let layout = SpaceLayout {
			id: self.space_id,
			space,
			space_items,
			groups,
		};

		Ok(layout)
	}
}

crate::register_library_query!(SpaceLayoutQuery, "spaces.get_layout");
