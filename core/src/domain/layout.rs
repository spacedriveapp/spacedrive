//! Layouts - optional named sidebar workspaces
//!
//! A library always has one compatible backing layout. Additional layouts let a
//! person save a distinct set of pins and custom sections without changing the
//! library or turning storage topology into navigation.

use crate::domain::addressing::SdPath;
use crate::domain::resource::Identifiable;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A Layout defines a sidebar layout and filtering context
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Layout {
	/// Unique identifier
	pub id: Uuid,

	/// Human-friendly name (e.g., "Default", "Work Files")
	pub name: String,

	/// Icon identifier (Phosphor icon name or emoji)
	pub icon: String,

	/// Color for visual identification (hex format: #RRGGBB)
	pub color: String,

	/// Sort order in layout switcher
	pub order: i32,

	/// Timestamps
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
}

impl Layout {
	/// Create a new layout
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

	/// Create the compatible backing layout for a library.
	pub fn create_default() -> Self {
		Self::new(
			"Default".to_string(),
			"Planet".to_string(),
			"#3B82F6".to_string(),
		)
	}
}

impl Identifiable for Layout {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"layout"
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::infra::db::entities::layout;
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let layout_models = layout::Entity::find()
			.filter(layout::Column::Uuid.is_in(ids.to_vec()))
			.all(db)
			.await?;

		Ok(layout_models
			.into_iter()
			.map(|s| Layout {
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

/// A LayoutGroup is a collapsible section in the sidebar
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutGroup {
	/// Unique identifier
	pub id: Uuid,

	/// Layout this group belongs to
	pub layout_id: Uuid,

	/// Group name (e.g., "Quick Access", "MacBook Pro")
	pub name: String,

	/// Type of group (determines content and behavior)
	pub group_type: GroupType,

	/// Whether group is collapsed
	pub is_collapsed: bool,

	/// Sort order within layout
	pub order: i32,

	/// Timestamp
	pub created_at: DateTime<Utc>,
}

impl LayoutGroup {
	/// Create a new group
	pub fn new(layout_id: Uuid, name: String, group_type: GroupType) -> Self {
		Self {
			id: Uuid::new_v4(),
			layout_id,
			name,
			group_type,
			is_collapsed: false,
			order: 0,
			created_at: Utc::now(),
		}
	}

	/// Create a Quick Access group
	pub fn create_quick_access(layout_id: Uuid) -> Self {
		Self::new(
			layout_id,
			"Quick Access".to_string(),
			GroupType::QuickAccess,
		)
	}

	/// Create a Device group
	pub fn create_device(layout_id: Uuid, device_id: Uuid, device_name: String) -> Self {
		Self::new(layout_id, device_name, GroupType::Device { device_id })
	}
}

impl Identifiable for LayoutGroup {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"layout_group"
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>>
	where
		Self: Sized,
	{
		use crate::infra::db::entities::{layout, layout_group};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let group_models = layout_group::Entity::find()
			.filter(layout_group::Column::Uuid.is_in(ids.to_vec()))
			.all(db)
			.await?;

		let mut results = Vec::new();

		for group_model in group_models {
			// Fetch parent layout to get layout_id (UUID)
			let layout_model = layout::Entity::find_by_id(group_model.layout_id)
				.one(db)
				.await?;

			let layout_id = layout_model.map(|s| s.uuid).unwrap_or(group_model.uuid);

			let group_type: GroupType =
				serde_json::from_str(&group_model.group_type).map_err(|e| {
					crate::common::errors::CoreError::Other(anyhow::anyhow!(
						"Failed to parse group_type: {}",
						e
					))
				})?;

			results.push(LayoutGroup {
				id: group_model.uuid,
				layout_id,
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

/// Types of groups that can appear in a layout
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum GroupType {
	/// Fixed quick navigation (Overview, Recents, Favorites)
	QuickAccess,

	/// Device with its volumes as children
	Device { device_id: Uuid },

	/// All devices (library and paired) across the system
	Devices,

	/// All volumes across all devices
	Volumes,

	/// Tag collection
	Tags,

	/// Archive data sources (email, notes, bookmarks, etc.)
	Sources,

	/// Cloud storage providers
	Cloud,

	/// User-defined custom group
	Custom,
}

/// An item within a layout (can be layout-level or within a group)
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutItem {
	/// Unique identifier
	pub id: Uuid,

	/// Layout this item belongs to
	pub layout_id: Uuid,

	/// Group this item belongs to (None = layout-level item)
	pub group_id: Option<Uuid>,

	/// Type discriminant (for quick type checking)
	pub item_type: ItemType,

	/// Sort order within layout or group
	pub order: i32,

	/// Timestamp
	pub created_at: DateTime<Utc>,

	/// Resolved file data for Path items (populated by get_sidebar query)
	#[serde(skip_serializing_if = "Option::is_none")]
	pub resolved_file: Option<Box<super::File>>,
}

impl LayoutItem {
	/// Create a new item within a group
	pub fn new(layout_id: Uuid, group_id: Uuid, item_type: ItemType) -> Self {
		Self {
			id: Uuid::new_v4(),
			layout_id,
			group_id: Some(group_id),
			item_type,
			order: 0,
			created_at: Utc::now(),
			resolved_file: None,
		}
	}

	/// Create a new layout-level item (not in any group)
	pub fn new_layout_level(layout_id: Uuid, item_type: ItemType) -> Self {
		Self {
			id: Uuid::new_v4(),
			layout_id,
			group_id: None,
			item_type,
			order: 0,
			created_at: Utc::now(),
			resolved_file: None,
		}
	}

	/// Create an Overview item
	pub fn create_overview(layout_id: Uuid, group_id: Uuid) -> Self {
		Self::new(layout_id, group_id, ItemType::Overview)
	}

	/// Create a Recents item
	pub fn create_recents(layout_id: Uuid, group_id: Uuid) -> Self {
		Self::new(layout_id, group_id, ItemType::Recents)
	}

	/// Create a Favorites item
	pub fn create_favorites(layout_id: Uuid, group_id: Uuid) -> Self {
		Self::new(layout_id, group_id, ItemType::Favorites)
	}

	/// Create a Path item (arbitrary SdPath)
	pub fn create_path(layout_id: Uuid, group_id: Uuid, sd_path: SdPath) -> Self {
		Self::new(layout_id, group_id, ItemType::Path { sd_path })
	}

	/// Create a layout-level Path item (pinned shortcut)
	pub fn create_layout_level_path(layout_id: Uuid, sd_path: SdPath) -> Self {
		Self::new_layout_level(layout_id, ItemType::Path { sd_path })
	}
}

impl Identifiable for LayoutItem {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"layout_item"
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
		use crate::infra::db::entities::{layout, layout_group, layout_item};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let item_models = layout_item::Entity::find()
			.filter(layout_item::Column::Uuid.is_in(ids.to_vec()))
			.all(db)
			.await?;

		let mut results = Vec::new();

		for item_model in item_models {
			// Fetch parent layout to get layout_id (UUID)
			let layout_model = layout::Entity::find_by_id(item_model.layout_id)
				.one(db)
				.await?;

			let layout_id = layout_model.map(|s| s.uuid).unwrap_or(item_model.uuid);

			let item_type: ItemType = serde_json::from_str(&item_model.item_type).map_err(|e| {
				crate::common::errors::CoreError::Other(anyhow::anyhow!(
					"Failed to parse item_type: {}",
					e
				))
			})?;

			// Look up group UUID from group_id if present
			let group_id = if let Some(gid) = item_model.group_id {
				layout_group::Entity::find_by_id(gid)
					.one(db)
					.await?
					.map(|g| g.uuid)
			} else {
				None
			};

			// Resolving a pinned file needs the volume index, which an event
			// emission does not have. `layouts.get_sidebar` is where the file
			// comes from; this carries the pin.
			let resolved_file = None;

			results.push(LayoutItem {
				id: item_model.uuid,
				layout_id,
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

	/// Storage volume
	Volume { volume_id: Uuid },

	/// Tag filter
	Tag { tag_id: Uuid },

	/// Any arbitrary path (dragged from explorer)
	Path { sd_path: SdPath },

	/// All archive data sources screen
	Sources,

	/// Specific archive data source
	Source { source_id: String },

	/// Redundancy awareness dashboard
	Redundancy,

	/// A predefined search pinned to the sidebar, identified ahead of time
	/// by indexing (never pattern-matched at query time). The slug names a
	/// built-in collection definition, e.g. "screenshots".
	Collection { slug: String },

	/// The storage analyzer: every volume rendered as a size breakdown,
	/// each drilling into a full-fidelity view over the index rollups.
	Analyzer,
}
/// Complete sidebar layout for a layout
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SidebarLayout {
	/// Unique identifier (same as layout.id for cache matching)
	pub id: Uuid,

	/// The layout
	pub layout: Layout,

	/// Layout-level items (pinned shortcuts, no group)
	pub layout_items: Vec<LayoutItem>,

	/// Groups with their items
	pub groups: Vec<LayoutGroupWithItems>,
}

impl SidebarLayout {
	/// Construct SidebarLayout from layout IDs (for resource manager)
	pub async fn from_layout_ids(
		db: &sea_orm::DatabaseConnection,
		layout_ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>> {
		use crate::infra::db::entities::{layout, layout_group, layout_item};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

		let mut layouts = Vec::new();

		for &layout_id in layout_ids {
			// Get layout
			let layout_model = layout::Entity::find()
				.filter(layout::Column::Uuid.eq(layout_id))
				.one(db)
				.await?;

			let Some(layout_model) = layout_model else {
				continue;
			};

			let layout = Layout {
				id: layout_model.uuid,
				name: layout_model.name,
				icon: layout_model.icon,
				color: layout_model.color,
				order: layout_model.order,
				created_at: layout_model.created_at.into(),
				updated_at: layout_model.updated_at.into(),
			};

			// Get layout-level items
			let layout_item_models = layout_item::Entity::find()
				.filter(layout_item::Column::LayoutId.eq(layout_model.id))
				.filter(layout_item::Column::GroupId.is_null())
				.order_by_asc(layout_item::Column::Order)
				.all(db)
				.await?;

			let mut layout_items = Vec::new();
			for item_model in layout_item_models {
				let item_type: ItemType =
					serde_json::from_str(&item_model.item_type).map_err(|e| {
						crate::common::errors::CoreError::Other(anyhow::anyhow!(
							"Failed to parse item_type: {}",
							e
						))
					})?;

				layout_items.push(LayoutItem {
					id: item_model.uuid,
					layout_id,
					group_id: None,
					item_type,
					order: item_model.order,
					created_at: item_model.created_at.into(),
					resolved_file: None,
				});
			}

			// Get groups with items
			let group_models = layout_group::Entity::find()
				.filter(layout_group::Column::LayoutId.eq(layout_model.id))
				.order_by_asc(layout_group::Column::Order)
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

				let group = LayoutGroup {
					id: group_model.uuid,
					layout_id,
					name: group_model.name,
					group_type,
					is_collapsed: group_model.is_collapsed,
					order: group_model.order,
					created_at: group_model.created_at.into(),
				};

				// Get items for this group
				let item_models = layout_item::Entity::find()
					.filter(layout_item::Column::GroupId.eq(Some(group_model.id)))
					.order_by_asc(layout_item::Column::Order)
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

					items.push(LayoutItem {
						id: item_model.uuid,
						layout_id,
						group_id: Some(group_model.uuid),
						item_type,
						order: item_model.order,
						created_at: item_model.created_at.into(),
						resolved_file: None,
					});
				}

				groups.push(LayoutGroupWithItems { group, items });
			}

			layouts.push(SidebarLayout {
				id: layout_id,
				layout,
				layout_items,
				groups,
			});
		}

		Ok(layouts)
	}
}

impl Identifiable for SidebarLayout {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"sidebar_layout"
	}

	fn sync_dependencies() -> &'static [&'static str] {
		&["layout", "layout_group", "layout_item"]
	}

	async fn route_from_dependency(
		db: &sea_orm::DatabaseConnection,
		dependency_type: &str,
		dependency_id: Uuid,
	) -> crate::common::errors::Result<Vec<Uuid>> {
		use crate::infra::db::entities::{layout, layout_group, layout_item};
		use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

		let layout_id = match dependency_type {
			// Pattern 1: Direct - SidebarLayout ID = Layout ID
			"layout" => dependency_id,

			// Pattern 3: Hierarchical rollup - navigate to parent Layout
			"layout_group" => {
				if let Some(group) = layout_group::Entity::find()
					.filter(layout_group::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
				{
					layout::Entity::find_by_id(group.layout_id)
						.one(db)
						.await?
						.map(|s| s.uuid)
						.unwrap_or(dependency_id)
				} else {
					dependency_id
				}
			}

			"layout_item" => {
				if let Some(item) = layout_item::Entity::find()
					.filter(layout_item::Column::Uuid.eq(dependency_id))
					.one(db)
					.await?
				{
					layout::Entity::find_by_id(item.layout_id)
						.one(db)
						.await?
						.map(|s| s.uuid)
						.unwrap_or(dependency_id)
				} else {
					dependency_id
				}
			}

			// When a volume, tag, or device changes, invalidate all layouts
			// (they will be re-queried with fresh JOINed data)
			"volume" | "tag" | "device" => {
				// Return all layout IDs to invalidate all layouts
				let all_layouts = layout::Entity::find().all(db).await?;
				return Ok(all_layouts.into_iter().map(|s| s.uuid).collect());
			}

			_ => return Ok(vec![]),
		};

		Ok(vec![layout_id])
	}

	async fn from_ids(
		db: &sea_orm::DatabaseConnection,
		ids: &[Uuid],
	) -> crate::common::errors::Result<Vec<Self>> {
		SidebarLayout::from_layout_ids(db, ids).await
	}
}

/// A group with its items
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct LayoutGroupWithItems {
	/// The group
	pub group: LayoutGroup,

	/// Items in this group (sorted by order)
	pub items: Vec<LayoutItem>,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_layout_creation() {
		let layout = Layout::new(
			"Test Layout".to_string(),
			"Folder".to_string(),
			"#FF0000".to_string(),
		);

		assert_eq!(layout.name, "Test Layout");
		assert_eq!(layout.icon, "Folder");
		assert_eq!(layout.color, "#FF0000");
		assert_eq!(layout.order, 0);
	}

	#[test]
	fn test_default_layout() {
		let layout = Layout::create_default();

		assert_eq!(layout.name, "Default");
		assert_eq!(layout.icon, "Planet");
		assert_eq!(layout.color, "#3B82F6");
	}

	#[test]
	fn test_color_validation() {
		assert!(Layout::validate_color("#3B82F6"));
		assert!(Layout::validate_color("#FFFFFF"));
		assert!(!Layout::validate_color("3B82F6")); // Missing #
		assert!(!Layout::validate_color("#FFF")); // Too short
		assert!(!Layout::validate_color("#3B82F6A")); // Too long
	}

	#[test]
	fn test_group_creation() {
		let layout_id = Uuid::new_v4();
		let group = LayoutGroup::create_quick_access(layout_id);

		assert_eq!(group.layout_id, layout_id);
		assert_eq!(group.name, "Quick Access");
		assert_eq!(group.group_type, GroupType::QuickAccess);
		assert!(!group.is_collapsed);
	}

	#[test]
	fn test_device_group_creation() {
		let layout_id = Uuid::new_v4();
		let device_id = Uuid::new_v4();
		let group = LayoutGroup::create_device(layout_id, device_id, "MacBook Pro".to_string());

		assert_eq!(group.name, "MacBook Pro");
		assert_eq!(group.group_type, GroupType::Device { device_id });
	}

	#[test]
	fn test_item_creation() {
		let layout_id = Uuid::new_v4();
		let group_id = Uuid::new_v4();

		let overview = LayoutItem::create_overview(layout_id, group_id);
		assert_eq!(overview.item_type, ItemType::Overview);
		assert_eq!(overview.group_id, Some(group_id));

		let recents = LayoutItem::create_recents(layout_id, group_id);
		assert_eq!(recents.item_type, ItemType::Recents);

		let favorites = LayoutItem::create_favorites(layout_id, group_id);
		assert_eq!(favorites.item_type, ItemType::Favorites);
	}

	#[test]
	fn test_layout_level_item() {
		let layout_id = Uuid::new_v4();
		let sd_path = crate::domain::SdPath::Physical {
			device_slug: "macbook".to_string(),
			path: "/Users/me/Documents".into(),
		};

		let item = LayoutItem::create_layout_level_path(layout_id, sd_path);
		assert_eq!(item.layout_id, layout_id);
		assert_eq!(item.group_id, None);
		assert!(matches!(item.item_type, ItemType::Path { .. }));
	}
}

// Register simple resources (single table, no dependencies)
crate::register_resource!(Layout);
crate::register_resource!(LayoutGroup);
crate::register_resource!(LayoutItem);

// Register SidebarLayout as a virtual resource (has dependencies on layout, layout_group, layout_item)
crate::register_resource!(SidebarLayout, virtual);
