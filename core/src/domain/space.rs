//! Spaces - Arc-browser-inspired sidebar organization system
//!
//! Spaces allow users to create custom sidebar layouts with device-aware groups,
//! sortable items, and context-based filtering. Each Space defines how the
//! sidebar is organized and what items are visible.

use crate::domain::addressing::SdPath;
use crate::domain::resource::Identifiable;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A Space defines a sidebar layout and filtering context
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Space {
	/// Unique identifier
	pub id: Uuid,

	/// Human-friendly name (e.g., "All Devices", "Work Files")
	pub name: String,

	/// Icon identifier (Phosphor icon name or emoji)
	pub icon: String,

	/// Color for visual identification (hex format: #RRGGBB)
	pub color: String,

	/// Sort order in space switcher
	pub order: i32,

	/// Timestamps
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl Space {
	/// Create a new space
	pub fn new(name: String, icon: String, color: String) -> Self {
		let now = Utc::now();
		Self {
			id: Uuid::new_v4(),
			name,
			icon,
			color,
			order: 0,
			created_at: now,
			updated_at: now,
		}
	}

	/// Validate color format
	pub fn validate_color(color: &str) -> bool {
		color.starts_with('#') && color.len() == 7
	}

	/// Create a default "All Devices" space
	pub fn create_default() -> Self {
		Self::new(
			"All Devices".to_string(),
			"Planet".to_string(),
			"#3B82F6".to_string(),
		)
	}
}

impl Identifiable for Space {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"space"
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::infra::db::entities::space;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let space_models = space::Entity::find()
			.filter(space::Column::Uuid.is_in(ids.to_vec()))
			.all(db)
			.await?;

		Ok(space_models
			.into_iter()
			.map(|s| Space {
				id: s.uuid,
				name: s.name,
				icon: s.icon,
				color: s.color,
				order: s.order,
				created_at: s.created_at.into(),
				updated_at: s.updated_at.into(),
			})
			.collect())
	}
}

/// A SpaceGroup is a collapsible section in the sidebar
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceGroup {
	/// Unique identifier
	pub id: Uuid,

	/// Space this group belongs to
	pub space_id: Uuid,

	/// Group name (e.g., "Quick Access", "MacBook Pro")
	pub name: String,

	/// Type of group (determines content and behavior)
	pub group_type: GroupType,

	/// Whether group is collapsed
	pub is_collapsed: bool,

	/// Sort order within space
	pub order: i32,

	/// Timestamp
	pub created_at: DateTime<Utc>,
}

impl SpaceGroup {
	/// Create a new group
	pub fn new(space_id: Uuid, name: String, group_type: GroupType) -> Self {
		Self {
			id: Uuid::new_v4(),
			space_id,
			name,
			group_type,
			is_collapsed: false,
			order: 0,
			created_at: Utc::now(),
		}
	}

	/// Create a Quick Access group
	pub fn create_quick_access(space_id: Uuid) -> Self {
		Self::new(space_id, "Quick Access".to_string(), GroupType::QuickAccess)
	}

	/// Create a Device group
	pub fn create_device(space_id: Uuid, device_id: Uuid, device_name: String) -> Self {
		Self::new(space_id, device_name, GroupType::Device { device_id })
	}
}

impl Identifiable for SpaceGroup {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"space_group"
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::infra::db::entities::{space, space_group};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let group_models = space_group::Entity::find()
			.filter(space_group::Column::Uuid.is_in(ids.to_vec()))
			.all(db)
			.await?;

		let mut results = Vec::new();

		for group_model in group_models {
			// Fetch parent space to get space_id (UUID)
			let space_model = space::Entity::find_by_id(group_model.space_id)
				.one(db)
				.await?;

			let space_id = space_model.map(|s| s.uuid).unwrap_or(group_model.uuid);

			let group_type: GroupType =
				serde_json::from_str(&group_model.group_type).map_err(|e| {
					crate::common::errors::CoreError::Other(anyhow::anyhow!(
						"Failed to parse group_type: {}",
						e
					))
				})?;

			results.push(SpaceGroup {
				id: group_model.uuid,
				space_id,
				name: group_model.name,
				group_type,
				is_collapsed: group_model.is_collapsed,
				order: group_model.order,
				created_at: group_model.created_at.into(),
			});
		}

		Ok(results)
	}
}

/// Types of groups that can appear in a space
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum GroupType {
	/// Fixed quick navigation (Overview, Recents, Favorites)
	QuickAccess,

	/// Device with its volumes and locations as children
	Device { device_id: Uuid },

	/// All devices (library and paired) across the system
	Devices,

	/// All locations across all devices
	Locations,

	/// All volumes across all devices
	Volumes,

	/// Tag collection
	Tags,

	/// Cloud storage providers
	Cloud,

	/// User-defined custom group
	Custom,
}

/// An item within a space (can be space-level or within a group)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceItem {
	/// Unique identifier
	pub id: Uuid,

	/// Space this item belongs to
	pub space_id: Uuid,

	/// Group this item belongs to (None = space-level item)
	pub group_id: Option<Uuid>,

	/// Type discriminant (for quick type checking)
	pub item_type: ItemType,

	/// Sort order within space or group
	pub order: i32,

	/// Timestamp
	pub created_at: DateTime<Utc>,

	/// Resolved file data for Path items (populated by get_layout query)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub resolved_file: Option<Box<super::File>>,
}

impl SpaceItem {
	/// Create a new item within a group
	pub fn new(space_id: Uuid, group_id: Uuid, item_type: ItemType) -> Self {
		Self {
			id: Uuid::new_v4(),
			space_id,
			group_id: Some(group_id),
			item_type,
			order: 0,
			created_at: Utc::now(),
			resolved_file: None,
		}
	}

	/// Create a new space-level item (not in any group)
	pub fn new_space_level(space_id: Uuid, item_type: ItemType) -> Self {
		Self {
			id: Uuid::new_v4(),
			space_id,
			group_id: None,
			item_type,
			order: 0,
			created_at: Utc::now(),
			resolved_file: None,
		}
	}

	/// Create an Overview item
	pub fn create_overview(space_id: Uuid, group_id: Uuid) -> Self {
		Self::new(space_id, group_id, ItemType::Overview)
	}

	/// Create a Recents item
	pub fn create_recents(space_id: Uuid, group_id: Uuid) -> Self {
		Self::new(space_id, group_id, ItemType::Recents)
	}

	/// Create a Favorites item
	pub fn create_favorites(space_id: Uuid, group_id: Uuid) -> Self {
		Self::new(space_id, group_id, ItemType::Favorites)
	}

	/// Create a Location item
	pub fn create_location(space_id: Uuid, group_id: Uuid, location_id: Uuid) -> Self {
		Self::new(space_id, group_id, ItemType::Location { location_id })
	}

	/// Create a Path item (arbitrary SdPath)
	pub fn create_path(space_id: Uuid, group_id: Uuid, sd_path: SdPath) -> Self {
		Self::new(space_id, group_id, ItemType::Path { sd_path })
	}

	/// Create a space-level Path item (pinned shortcut)
	pub fn create_space_level_path(space_id: Uuid, sd_path: SdPath) -> Self {
		Self::new_space_level(space_id, ItemType::Path { sd_path })
	}
}

impl Identifiable for SpaceItem {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"space_item"
	}

	fn no_merge_fields() -> &'static [&'static str] {
		&["resolved_file"]
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::infra::db::entities::{entry, space, space_group, space_item};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let item_models = space_item::Entity::find()
			.filter(space_item::Column::Uuid.is_in(ids.to_vec()))
			.all(db)
			.await?;

		let mut results = Vec::new();

		for item_model in item_models {
			// Fetch parent space to get space_id (UUID)
			let space_model = space::Entity::find_by_id(item_model.space_id)
				.one(db)
				.await?;

			let space_id = space_model.map(|s| s.uuid).unwrap_or(item_model.uuid);

			let item_type: ItemType = serde_json::from_str(&item_model.item_type).map_err(|e| {
				crate::common::errors::CoreError::Other(anyhow::anyhow!(
					"Failed to parse item_type: {}",
					e
				))
			})?;

			// Look up group UUID from group_id if present
			let group_id = if let Some(gid) = item_model.group_id {
				space_group::Entity::find_by_id(gid)
					.one(db)
					.await?
					.map(|g| g.uuid)
			} else {
				None
			};

			// Build resolved_file if entry_uuid exists
			let resolved_file = if let Some(entry_uuid) = item_model.entry_uuid {
				let entry_model = entry::Entity::find()
					.filter(entry::Column::Uuid.eq(entry_uuid))
					.one(db)
					.await?;

				if let Some(entry_model) = entry_model {
					super::file::File::from_entry_model_with_item_type(entry_model, &item_type, db)
						.await
						.map(Box::new)
				} else {
					None
				}
			} else {
				None
			};

			results.push(SpaceItem {
				id: item_model.uuid,
				space_id,
				group_id,
				item_type,
				order: item_model.order,
				created_at: item_model.created_at.into(),
				resolved_file,
			});
		}

		Ok(results)
	}
}

/// Types of items that can appear in a group
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum ItemType {
	/// Overview screen (fixed)
	Overview,

	/// Recent files (fixed)
	Recents,

	/// Favorited files (fixed)
	Favorites,

	/// File kinds (images, videos, audio, etc.)
	FileKinds,

	/// Indexed location
	Location { location_id: Uuid },

	/// Storage volume (with locations as children)
	Volume { volume_id: Uuid },

	/// Tag filter
	Tag { tag_id: Uuid },

	/// Any arbitrary path (dragged from explorer)
	Path { sd_path: SdPath },
}
/// Complete sidebar layout for a space
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceLayout {
	/// Unique identifier (same as space.id for cache matching)
	pub id: Uuid,

	/// The space
	pub space: Space,

	/// Space-level items (pinned shortcuts, no group)
	pub space_items: Vec<SpaceItem>,

	/// Groups with their items
	pub groups: Vec<SpaceGroupWithItems>,
}

impl SpaceLayout {
	/// Construct SpaceLayout from space IDs (for resource manager)
	pub async fn from_space_ids(
		db: &sea_orm::DatabaseConnection,
		space_ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>> {
		use crate::infra::db::entities::{space, space_group, space_item};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

		let mut layouts = Vec::new();

		for &space_id in space_ids {
			// Get space
			let space_model = space::Entity::find()
				.filter(space::Column::Uuid.eq(space_id))
				.one(db)
				.await?;

			let Some(space_model) = space_model else {
				continue;
			};

			let space = Space {
				id: space_model.uuid,
				name: space_model.name,
				icon: space_model.icon,
				color: space_model.color,
				order: space_model.order,
				created_at: space_model.created_at.into(),
				updated_at: space_model.updated_at.into(),
			};

			// Get space-level items
			let space_item_models = space_item::Entity::find()
				.filter(space_item::Column::SpaceId.eq(space_model.id))
				.filter(space_item::Column::GroupId.is_null())
				.order_by_asc(space_item::Column::Order)
				.all(db)
				.await?;

			let mut space_items = Vec::new();
			for item_model in space_item_models {
				let item_type: ItemType =
					serde_json::from_str(&item_model.item_type).map_err(|e| {
						crate::common::errors::CoreError::Other(anyhow::anyhow!(
							"Failed to parse item_type: {}",
							e
						))
					})?;

				space_items.push(SpaceItem {
					id: item_model.uuid,
					space_id,
					group_id: None,
					item_type,
					order: item_model.order,
					created_at: item_model.created_at.into(),
					resolved_file: None,
				});
			}

			// Get groups with items
			let group_models = space_group::Entity::find()
				.filter(space_group::Column::SpaceId.eq(space_model.id))
				.order_by_asc(space_group::Column::Order)
				.all(db)
				.await?;

			let mut groups = Vec::new();
			for group_model in group_models {
				let group_type: GroupType =
					serde_json::from_str(&group_model.group_type).map_err(|e| {
						crate::common::errors::CoreError::Other(anyhow::anyhow!(
							"Failed to parse group_type: {}",
							e
						))
					})?;

				let group = SpaceGroup {
					id: group_model.uuid,
					space_id,
					name: group_model.name,
					group_type,
					is_collapsed: group_model.is_collapsed,
					order: group_model.order,
					created_at: group_model.created_at.into(),
				};

				// Get items for this group
				let item_models = space_item::Entity::find()
					.filter(space_item::Column::GroupId.eq(Some(group_model.id)))
					.order_by_asc(space_item::Column::Order)
					.all(db)
					.await?;

				let mut items = Vec::new();
				for item_model in item_models {
					let item_type: ItemType =
						serde_json::from_str(&item_model.item_type).map_err(|e| {
							crate::common::errors::CoreError::Other(anyhow::anyhow!(
								"Failed to parse item_type: {}",
								e
							))
						})?;

					items.push(SpaceItem {
						id: item_model.uuid,
						space_id,
						group_id: Some(group_model.uuid),
						item_type,
						order: item_model.order,
						created_at: item_model.created_at.into(),
						resolved_file: None,
					});
				}

				groups.push(SpaceGroupWithItems { group, items });
			}

			layouts.push(SpaceLayout {
				id: space_id,
				space,
				space_items,
				groups,
			});
		}

		Ok(layouts)
	}
}

impl Identifiable for SpaceLayout {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"space_layout"
	}

	fn sync_dependencies() -> &'static [&'static str] {
		&["space", "space_group", "space_item"]
	}

	async fn route_from_dependency(
		db: &sea_orm::DatabaseConnection,
		dependency_type: &str,
		dependency_id: Uuid,
	) -> crate::common::errors::Result<Vec<Uuid>> {
		use crate::infra::db::entities::{space, space_group, space_item};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let space_id = match dependency_type {
			// Pattern 1: Direct - SpaceLayout ID = Space ID
			"space" => dependency_id,

			// Pattern 3: Hierarchical rollup - navigate to parent Space
			"space_group" => {
				if let Some(group) = space_group::Entity::find()
					.filter(space_group::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
				{
					space::Entity::find_by_id(group.space_id)
						.one(db)
						.await?
						.map(|s| s.uuid)
						.unwrap_or(dependency_id)
				} else {
					dependency_id
				}
			}

			"space_item" => {
				if let Some(item) = space_item::Entity::find()
					.filter(space_item::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
				{
					space::Entity::find_by_id(item.space_id)
						.one(db)
						.await?
						.map(|s| s.uuid)
						.unwrap_or(dependency_id)
				} else {
					dependency_id
				}
			}

			// When location/volume/tag/device changes, invalidate all spaces
			// (they will be re-queried with fresh JOINed data)
			"location" | "volume" | "tag" | "device" => {
				// Return all space IDs to invalidate all layouts
				let all_spaces = space::Entity::find().all(db).await?;
				return Ok(all_spaces.into_iter().map(|s| s.uuid).collect());
			}

			_ => return Ok(vec![]),
		};

		Ok(vec![space_id])
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>> {
		SpaceLayout::from_space_ids(db, ids).await
	}
}

/// A group with its items
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceGroupWithItems {
	/// The group
	pub group: SpaceGroup,

	/// Items in this group (sorted by order)
	pub items: Vec<SpaceItem>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_space_creation() {
		let space = Space::new(
			"Test Space".to_string(),
			"Folder".to_string(),
			"#FF0000".to_string(),
		);

		assert_eq!(space.name, "Test Space");
		assert_eq!(space.icon, "Folder");
		assert_eq!(space.color, "#FF0000");
		assert_eq!(space.order, 0);
	}

	#[test]
	fn test_default_space() {
		let space = Space::create_default();

		assert_eq!(space.name, "All Devices");
		assert_eq!(space.icon, "Planet");
		assert_eq!(space.color, "#3B82F6");
	}

	#[test]
	fn test_color_validation() {
		assert!(Space::validate_color("#3B82F6"));
		assert!(Space::validate_color("#FFFFFF"));
		assert!(!Space::validate_color("3B82F6")); // Missing #
		assert!(!Space::validate_color("#FFF")); // Too short
		assert!(!Space::validate_color("#3B82F6A")); // Too long
	}

	#[test]
	fn test_group_creation() {
		let space_id = Uuid::new_v4();
		let group = SpaceGroup::create_quick_access(space_id);

		assert_eq!(group.space_id, space_id);
		assert_eq!(group.name, "Quick Access");
		assert_eq!(group.group_type, GroupType::QuickAccess);
		assert!(!group.is_collapsed);
	}

	#[test]
	fn test_device_group_creation() {
		let space_id = Uuid::new_v4();
		let device_id = Uuid::new_v4();
		let group = SpaceGroup::create_device(space_id, device_id, "MacBook Pro".to_string());

		assert_eq!(group.name, "MacBook Pro");
		assert_eq!(group.group_type, GroupType::Device { device_id });
	}

	#[test]
	fn test_item_creation() {
		let space_id = Uuid::new_v4();
		let group_id = Uuid::new_v4();

		let overview = SpaceItem::create_overview(space_id, group_id);
		assert_eq!(overview.item_type, ItemType::Overview);
		assert_eq!(overview.group_id, Some(group_id));

		let recents = SpaceItem::create_recents(space_id, group_id);
		assert_eq!(recents.item_type, ItemType::Recents);

		let favorites = SpaceItem::create_favorites(space_id, group_id);
		assert_eq!(favorites.item_type, ItemType::Favorites);
	}

	#[test]
	fn test_space_level_item() {
		let space_id = Uuid::new_v4();
		let sd_path = crate::domain::SdPath::Physical {
			device_slug: "macbook".to_string(),
			path: "/Users/me/Documents".into(),
		};

		let item = SpaceItem::create_space_level_path(space_id, sd_path);
		assert_eq!(item.space_id, space_id);
		assert_eq!(item.group_id, None);
		assert!(matches!(item.item_type, ItemType::Path { .. }));
	}
}

// Register simple resources (single table, no dependencies)
crate::register_resource!(Space);
crate::register_resource!(SpaceGroup);
crate::register_resource!(SpaceItem);

// Register SpaceLayout as a virtual resource (has dependencies on space, space_group, space_item)
crate::register_resource!(SpaceLayout, virtual);
