//! The navigation focus resource and the in-memory registry behind it.

use std::{collections::HashMap, sync::RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::domain::{resource::Identifiable, SdPath};

/// The group a window joins when it names none.
pub const DEFAULT_GROUP: &str = "default";

/// Namespace for deriving a stable resource id from a group name — focus rows
/// are keyed by group, and the id must be the same for every client.
const FOCUS_NAMESPACE: Uuid = Uuid::from_bytes([
	0x2d, 0x71, 0x4a, 0x90, 0xc3, 0x18, 0x4f, 0x6b, 0x85, 0xe2, 0x0a, 0x37, 0xd9, 0x64, 0x1b, 0x5c,
]);

/// Where one focus group is currently looking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NavigationFocus {
	pub id: Uuid,
	pub group: String,
	/// The directory in view, or `None` when the publisher is showing
	/// something that has no path (a search, a tag, an empty window).
	pub path: Option<SdPath>,
	pub library_id: Option<Uuid>,
	/// Free-form label naming the window that published this, so a client can
	/// recognize and ignore its own echo.
	pub origin: Option<String>,
	pub updated_at: DateTime<Utc>,
}

impl NavigationFocus {
	/// The resource type string clients filter events by, available without
	/// importing [`Identifiable`].
	pub const RESOURCE_TYPE: &'static str = "navigation_focus";

	pub fn id_for_group(group: &str) -> Uuid {
		Uuid::new_v5(&FOCUS_NAMESPACE, group.as_bytes())
	}

	/// An empty focus for a group nobody has published to yet.
	pub fn empty(group: String) -> Self {
		Self {
			id: Self::id_for_group(&group),
			group,
			path: None,
			library_id: None,
			origin: None,
			updated_at: Utc::now(),
		}
	}
}

impl Identifiable for NavigationFocus {
	fn id(&self) -> Uuid {
		self.id
	}

	fn resource_type() -> &'static str
	where
		Self: Sized,
	{
		Self::RESOURCE_TYPE
	}

	fn no_merge_fields() -> &'static [&'static str]
	where
		Self: Sized,
	{
		// A focus row is replaced wholesale: merging a new path into an old
		// one would resurrect the library or origin of a window that has
		// since navigated away.
		&["path", "library_id", "origin"]
	}
}

crate::register_resource!(NavigationFocus);

/// Every focus group on this machine. Last write wins within a group.
#[derive(Default)]
pub struct FocusRegistry {
	groups: RwLock<HashMap<String, NavigationFocus>>,
}

impl FocusRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	/// The current focus for `group`, or `None` if nothing has published to it.
	pub fn get(&self, group: &str) -> Option<NavigationFocus> {
		self.groups
			.read()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
			.get(group)
			.cloned()
	}

	/// Record `focus` as the group's current position, returning it when the
	/// position actually moved. A republish of the same path returns `None` so
	/// callers can skip the event: an explorer that refetches a listing must
	/// not wake every follower.
	pub fn set(&self, focus: NavigationFocus) -> Option<NavigationFocus> {
		let mut groups = self
			.groups
			.write()
			.unwrap_or_else(|poisoned| poisoned.into_inner());
		if let Some(current) = groups.get(&focus.group) {
			if current.path == focus.path && current.library_id == focus.library_id {
				return None;
			}
		}
		groups.insert(focus.group.clone(), focus.clone());
		Some(focus)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::path::PathBuf;

	fn focus(group: &str, path: &str) -> NavigationFocus {
		NavigationFocus {
			path: Some(SdPath::Physical {
				device_slug: "device".into(),
				path: PathBuf::from(path),
			}),
			..NavigationFocus::empty(group.to_string())
		}
	}

	#[test]
	fn group_ids_are_stable_and_distinct() {
		assert_eq!(
			NavigationFocus::id_for_group("default"),
			NavigationFocus::id_for_group("default")
		);
		assert_ne!(
			NavigationFocus::id_for_group("default"),
			NavigationFocus::id_for_group("second-window")
		);
	}

	#[test]
	fn republishing_the_same_path_reports_no_move() {
		let registry = FocusRegistry::new();
		assert!(registry.set(focus("default", "/photos")).is_some());
		assert!(registry.set(focus("default", "/photos")).is_none());
		assert!(registry.set(focus("default", "/photos/2026")).is_some());
	}

	#[test]
	fn groups_do_not_share_a_position() {
		let registry = FocusRegistry::new();
		registry.set(focus("default", "/photos"));
		registry.set(focus("second", "/documents"));
		assert_eq!(
			registry.get("default").unwrap().path,
			focus("default", "/photos").path
		);
		assert_eq!(
			registry.get("second").unwrap().path,
			focus("second", "/documents").path
		);
		assert!(registry.get("third").is_none());
	}
}
