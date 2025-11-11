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
		Self::new(
			space_id,
			device_name,
			GroupType::Device { device_id },
		)
	}
}

impl Identifiable for SpaceGroup {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"space_group"
	}
}

/// Types of groups that can appear in a space
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
pub enum GroupType {
	/// Fixed quick navigation (Overview, Recents, Favorites)
	QuickAccess,

	/// Device with its volumes and locations as children
	Device { device_id: Uuid },

	/// All locations across all devices
	Locations,

	/// Tag collection
	Tags,

	/// Cloud storage providers
	Cloud,

	/// User-defined custom group
	Custom,
}

/// An item within a group
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SpaceItem {
	/// Unique identifier
	pub id: Uuid,

	/// Group this item belongs to
	pub group_id: Uuid,

	/// Type and data of this item
	pub item_type: ItemType,

	/// Sort order within group
	pub order: i32,

	/// Timestamp
	pub created_at: DateTime<Utc>,
}

impl SpaceItem {
	/// Create a new item
	pub fn new(group_id: Uuid, item_type: ItemType) -> Self {
		Self {
			id: Uuid::new_v4(),
			group_id,
			item_type,
			order: 0,
			created_at: Utc::now(),
		}
	}

	/// Create an Overview item
	pub fn create_overview(group_id: Uuid) -> Self {
		Self::new(group_id, ItemType::Overview)
	}

	/// Create a Recents item
	pub fn create_recents(group_id: Uuid) -> Self {
		Self::new(group_id, ItemType::Recents)
	}

	/// Create a Favorites item
	pub fn create_favorites(group_id: Uuid) -> Self {
		Self::new(group_id, ItemType::Favorites)
	}

	/// Create a Location item
	pub fn create_location(group_id: Uuid, location_id: Uuid) -> Self {
		Self::new(group_id, ItemType::Location { location_id })
	}

	/// Create a Path item (arbitrary SdPath)
	pub fn create_path(group_id: Uuid, sd_path: SdPath) -> Self {
		Self::new(group_id, ItemType::Path { sd_path })
	}
}

impl Identifiable for SpaceItem {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str {
		"space_item"
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
	/// The space
	pub space: Space,

	/// Groups with their items
	pub groups: Vec<SpaceGroupWithItems>,
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
		let group_id = Uuid::new_v4();

		let overview = SpaceItem::create_overview(group_id);
		assert_eq!(overview.item_type, ItemType::Overview);

		let recents = SpaceItem::create_recents(group_id);
		assert_eq!(recents.item_type, ItemType::Recents);

		let favorites = SpaceItem::create_favorites(group_id);
		assert_eq!(favorites.item_type, ItemType::Favorites);
	}
}
