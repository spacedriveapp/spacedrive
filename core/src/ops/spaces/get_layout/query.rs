use super::output::SpaceLayoutOutput;
use crate::domain::{
	addressing::SdPath, ContentKind, File, GroupType, ItemType, Space, SpaceGroup,
	SpaceGroupWithItems, SpaceItem, SpaceLayout,
};
use crate::infra::db::entities::{content_identity, entry, sidecar, space_item};
use crate::infra::query::{QueryError, QueryResult};
use crate::{context::CoreContext, infra::query::LibraryQuery};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
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

			// Resolve entry if entry_uuid is set
			let resolved_file = if let Some(entry_uuid) = item_model.entry_uuid {
				tracing::debug!(
					"Space item {} has entry_uuid: {}",
					item_model.uuid,
					entry_uuid
				);
				if let Ok(Some(entry_model)) = entry::Entity::find()
					.filter(entry::Column::Uuid.eq(entry_uuid))
					.one(db)
					.await
				{
					let file = build_file_from_entry(entry_model, &item_type, db)
						.await
						.map(Box::new);
					file
				} else {
					tracing::warn!("Entry {} not found for space item", entry_uuid);
					None
				}
			} else {
				tracing::debug!("Space item {} has no entry_uuid", item_model.uuid);
				None
			};

			space_items.push(SpaceItem {
				id: item_model.uuid,
				space_id: self.space_id,
				group_id: None,
				item_type,
				order: item_model.order,
				created_at: item_model.created_at.into(),
				resolved_file,
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

				// Resolve entry if entry_uuid is set
				let resolved_file = if let Some(entry_uuid) = item_model.entry_uuid {
					tracing::debug!(
						"Group item {} has entry_uuid: {}",
						item_model.uuid,
						entry_uuid
					);
					if let Ok(Some(entry_model)) = entry::Entity::find()
						.filter(entry::Column::Uuid.eq(entry_uuid))
						.one(db)
						.await
					{
						tracing::debug!("Found entry: name={}", entry_model.name);
						let file = build_file_from_entry(entry_model, &item_type, db)
							.await
							.map(Box::new);
						tracing::info!(
							"Built file for group item: {:?}",
							file.as_ref().map(|f| &f.name)
						);
						file
					} else {
						tracing::warn!("Entry {} not found for group item", entry_uuid);
						None
					}
				} else {
					None
				};

				items.push(SpaceItem {
					id: item_model.uuid,
					space_id: self.space_id,
					group_id: Some(group_model.uuid),
					item_type,
					order: item_model.order,
					created_at: item_model.created_at.into(),
					resolved_file,
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

/// Build a minimal File object from an entry model (for sidebar display)
async fn build_file_from_entry(
	entry_model: entry::Model,
	item_type: &ItemType,
	db: &DatabaseConnection,
) -> Option<File> {
	// Get the SdPath from item_type
	let sd_path = match item_type {
		ItemType::Path { sd_path } => sd_path.clone(),
		_ => return None,
	};

	// Get content identity if available
	let content_identity = if let Some(content_id) = entry_model.content_id {
		content_identity::Entity::find_by_id(content_id)
			.one(db)
			.await
			.ok()
			.flatten()
			.map(|ci| crate::domain::ContentIdentity {
				uuid: ci.uuid.unwrap_or_else(Uuid::new_v4),
				kind: ContentKind::from_id(ci.kind_id),
				content_hash: ci.content_hash,
				integrity_hash: ci.integrity_hash,
				mime_type_id: ci.mime_type_id,
				text_content: ci.text_content,
				total_size: ci.total_size,
				entry_count: ci.entry_count,
				first_seen_at: ci.first_seen_at,
				last_verified_at: ci.last_verified_at,
			})
	} else {
		None
	};

	// Get sidecars for thumbnails
	let sidecars = if let Some(ref ci) = content_identity {
		if let Some(uuid) = Some(ci.uuid) {
			sidecar::Entity::find()
				.filter(sidecar::Column::ContentUuid.eq(uuid))
				.all(db)
				.await
				.ok()
				.unwrap_or_default()
				.into_iter()
				.map(|s| crate::domain::Sidecar {
					id: s.id,
					content_uuid: s.content_uuid,
					kind: s.kind,
					variant: s.variant,
					format: s.format,
					status: s.status,
					size: s.size,
					created_at: s.created_at,
					updated_at: s.updated_at,
				})
				.collect()
		} else {
			Vec::new()
		}
	} else {
		Vec::new()
	};

	let mut file = File::from_entity_model(entry_model, sd_path);
	file.content_identity = content_identity;
	file.sidecars = sidecars;
	if let Some(ref ci) = file.content_identity {
		file.content_kind = ci.kind;
	}

	Some(file)
}
