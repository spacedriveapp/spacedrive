//! Resource trait for normalized cache system
//!
//! Resources are domain entities that can be cached and synced across devices.

use uuid::Uuid;

/// Trait for resources that can participate in the normalized cache
///
/// Any domain entity (Location, Tag, Album, File, etc.) that implements this
/// can automatically emit ResourceChanged events and be cached on the client.
pub trait Identifiable {
	/// Get the unique identifier
	fn id(&self) -> Uuid;

	/// Get the resource type string (e.g., "location", "tag", "album")
	fn resource_type() -> &'static str
	where
		Self: Sized;
}
