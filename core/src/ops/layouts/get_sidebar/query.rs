use super::output::SidebarLayoutOutput;
use crate::domain::{
	addressing::SdPath, File, GroupType, ItemType, Layout, LayoutGroup, LayoutGroupWithItems,
	LayoutItem, SidebarLayout,
};
use crate::infra::db::entities::layout_item;
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SidebarLayoutQueryInput {
	pub layout_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SidebarLayoutQuery {
	layout_id: Uuid,
}

impl LibraryQuery for SidebarLayoutQuery {
	type Input = SidebarLayoutQueryInput;
	type Output = SidebarLayoutOutput;

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

		// Get layout
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

		// Get layout-level items (no group)
		let layout_item_models = crate::infra::db::entities::layout_item::Entity::find()
			.filter(crate::infra::db::entities::layout_item::Column::LayoutId.eq(layout_model.id))
			.filter(crate::infra::db::entities::layout_item::Column::GroupId.is_null())
			.order_by_asc(crate::infra::db::entities::layout_item::Column::Order)
			.all(db)
			.await?;

		let mut layout_items = Vec::new();

		for item_model in layout_item_models {
			let item_type: ItemType = serde_json::from_str(&item_model.item_type)
				.map_err(|e| QueryError::Internal(format!("Failed to parse item_type: {}", e)))?;

			// Resolve entry if entry_uuid is set
			let resolved_file = if let Some(entry_uuid) = item_model.entry_uuid {
				tracing::debug!(
					"Layout item {} has entry_uuid: {}",
					item_model.uuid,
					entry_uuid
				);
				pinned_file(entry_uuid, &item_type, &context)
					.await
					.map(Box::new)
			} else {
				tracing::debug!("Layout item {} has no entry_uuid", item_model.uuid);
				None
			};

			layout_items.push(LayoutItem {
				id: item_model.uuid,
				layout_id: self.layout_id,
				group_id: None,
				item_type,
				order: item_model.order,
				created_at: item_model.created_at.into(),
				resolved_file,
			});
		}

		// Get groups for this layout
		let group_models = crate::infra::db::entities::layout_group::Entity::find()
			.filter(crate::infra::db::entities::layout_group::Column::LayoutId.eq(layout_model.id))
			.order_by_asc(crate::infra::db::entities::layout_group::Column::Order)
			.all(db)
			.await?;

		let mut groups = Vec::new();

		for group_model in group_models {
			// Parse group_type JSON
			let group_type: GroupType = serde_json::from_str(&group_model.group_type)
				.map_err(|e| QueryError::Internal(format!("Failed to parse group_type: {}", e)))?;

			let group = LayoutGroup {
				id: group_model.uuid,
				layout_id: self.layout_id,
				name: group_model.name,
				group_type,
				is_collapsed: group_model.is_collapsed,
				order: group_model.order,
				created_at: group_model.created_at.into(),
			};

			// Get items for this group
			let item_models = crate::infra::db::entities::layout_item::Entity::find()
				.filter(
					crate::infra::db::entities::layout_item::Column::GroupId
						.eq(Some(group_model.id)),
				)
				.order_by_asc(crate::infra::db::entities::layout_item::Column::Order)
				.all(db)
				.await?;

			let mut items = Vec::new();

			for item_model in item_models {
				// Parse item_type JSON
				let item_type: ItemType =
					serde_json::from_str(&item_model.item_type).map_err(|e| {
						QueryError::Internal(format!("Failed to parse item_type: {}", e))
					})?;

				// Resolve entry if entry_uuid is set
				let resolved_file = if let Some(entry_uuid) = item_model.entry_uuid {
					tracing::debug!(
						"Group item {} has entry_uuid: {}",
						item_model.uuid,
						entry_uuid
					);
					pinned_file(entry_uuid, &item_type, &context)
						.await
						.map(Box::new)
				} else {
					None
				};

				items.push(LayoutItem {
					id: item_model.uuid,
					layout_id: self.layout_id,
					group_id: Some(group_model.uuid),
					item_type,
					order: item_model.order,
					created_at: item_model.created_at.into(),
					resolved_file,
				});
			}

			groups.push(LayoutGroupWithItems { group, items });
		}

		let layout = SidebarLayout {
			id: self.layout_id,
			layout,
			layout_items,
			groups,
		};

		Ok(layout)
	}
}

crate::register_library_query!(SidebarLayoutQuery, "layouts.get_sidebar");

/// The file a layout item points at, for the sidebar.
///
/// A layout item stores the path it was pinned at and the uuid the entry had at
/// the time. The path is what makes it findable: the arena is keyed by path, so
/// a pinned file that has since been renamed resolves to nothing rather than to
/// whatever now sits at that path.
async fn pinned_file(
	entry_uuid: Uuid,
	item_type: &ItemType,
	context: &Arc<CoreContext>,
) -> Option<File> {
	let ItemType::Path { sd_path } = item_type else {
		return None;
	};
	let SdPath::Physical { path, .. } = sd_path else {
		return None;
	};

	let cache = context.volume_index();
	cache.ensure_restored(path).await;

	let index = cache.resolve_index(path);
	let mut index = index.write().await;
	let metadata = index.get_entry_ref(path)?;
	let kind = index.get_content_kind(path);
	drop(index);

	let mut file = File::from_arena(entry_uuid, &metadata, sd_path.clone());
	file.content_kind = kind;
	Some(file)
}
