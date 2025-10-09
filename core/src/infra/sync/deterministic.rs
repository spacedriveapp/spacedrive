//! Deterministic UUID generation for system-provided resources
//!
//! Used ONLY for system default tags and built-in resources that need
//! consistent UUIDs across all Spacedrive installations.
//!
//! WARNING: User-created tags should use random UUIDs to support the
//! semantic tagging system's polymorphic naming (multiple tags with 
//! the same name in different contexts).

use uuid::Uuid;

/// Usage Example:
/// ```rust,ignore
/// // System tags - use deterministic UUIDs for consistency
/// let system_tag = SemanticTag {
///     uuid: system_tags::SCREENSHOT_TAG_UUID.clone(),
///     canonical_name: "Screenshot".to_string(),
///     tag_type: TagType::System,
///     ...
/// };
/// 
/// // User tags - use random UUIDs for polymorphic naming
/// let user_tag = SemanticTag {
///     uuid: Uuid::new_v4(), // Random!
///     canonical_name: "Vacation".to_string(), 
///     namespace: Some("Travel".to_string()),
///     tag_type: TagType::Standard,
///     ...
/// };
/// ```

/// Namespace for tag UUIDs
pub const TAG_NAMESPACE: Uuid = Uuid::from_bytes([
	0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Namespace for album UUIDs
pub const ALBUM_NAMESPACE: Uuid = Uuid::from_bytes([
	0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Generate deterministic UUID for a system default tag
///
/// This should ONLY be used for built-in system tags that ship with
/// Spacedrive. User-created tags must use random UUIDs.
///
/// # Example
/// ```rust,ignore
/// // System tags that need consistent UUIDs
/// let uuid = deterministic_system_tag_uuid("System");
/// let uuid = deterministic_system_tag_uuid("Screenshot");
/// let uuid = deterministic_system_tag_uuid("Download");
/// ```
pub fn deterministic_system_tag_uuid(name: &str) -> Uuid {
	Uuid::new_v5(&TAG_NAMESPACE, name.as_bytes())
}

/// Generate deterministic UUID for an album name
/// System default tags that ship with every Spacedrive installation
/// 
/// These tags should be created during library initialization:
/// ```rust,ignore
/// // In library creation/initialization
/// for (name, uuid) in get_system_tags() {
///     let tag = SemanticTag {
///         uuid: uuid,
///         canonical_name: name,
///         tag_type: TagType::System,
///         namespace: Some("System".to_string()),
///         privacy_level: PrivacyLevel::Normal,
///         // ... other fields
///     };
///     tag.insert_or_ignore(db).await?;
/// }
/// ```
pub mod system_tags {
    use super::*;
    
    lazy_static::lazy_static! {
        // File type tags
        pub static ref SYSTEM_TAG_UUID: Uuid = deterministic_system_tag_uuid("System");
        pub static ref SCREENSHOT_TAG_UUID: Uuid = deterministic_system_tag_uuid("Screenshot");
        pub static ref DOWNLOAD_TAG_UUID: Uuid = deterministic_system_tag_uuid("Download");
        pub static ref DOCUMENT_TAG_UUID: Uuid = deterministic_system_tag_uuid("Document");
        pub static ref IMAGE_TAG_UUID: Uuid = deterministic_system_tag_uuid("Image");
        pub static ref VIDEO_TAG_UUID: Uuid = deterministic_system_tag_uuid("Video");
        pub static ref AUDIO_TAG_UUID: Uuid = deterministic_system_tag_uuid("Audio");
        
        // Special behavior tags  
        pub static ref HIDDEN_TAG_UUID: Uuid = deterministic_system_tag_uuid(".hidden");
        pub static ref ARCHIVE_TAG_UUID: Uuid = deterministic_system_tag_uuid(".archive");
        pub static ref FAVORITE_TAG_UUID: Uuid = deterministic_system_tag_uuid("Favorite");
    }
    
    /// Get all system tags for library initialization
    pub fn get_all_system_tags() -> Vec<(&'static str, Uuid)> {
        vec![
            ("System", SYSTEM_TAG_UUID.clone()),
            ("Screenshot", SCREENSHOT_TAG_UUID.clone()),
            ("Download", DOWNLOAD_TAG_UUID.clone()),
            ("Document", DOCUMENT_TAG_UUID.clone()),
            ("Image", IMAGE_TAG_UUID.clone()),
            ("Video", VIDEO_TAG_UUID.clone()),
            ("Audio", AUDIO_TAG_UUID.clone()),
            (".hidden", HIDDEN_TAG_UUID.clone()),
            (".archive", ARCHIVE_TAG_UUID.clone()),
            ("Favorite", FAVORITE_TAG_UUID.clone()),
        ]
    }
}

/// Generate deterministic UUID for a system default album
///
/// Similar to tags, only for system-provided albums.
pub fn deterministic_system_album_uuid(name: &str) -> Uuid {
	Uuid::new_v5(&ALBUM_NAMESPACE, name.as_bytes())
}

/// When to use deterministic vs random UUIDs:
/// 
/// USE DETERMINISTIC UUIDs FOR:
/// - System default tags that ship with Spacedrive
/// - Built-in tags referenced by code (e.g., HIDDEN_TAG_UUID)
/// - System albums like "Recent Imports" or "Quick Access"
/// - Any resource that needs the SAME UUID across ALL installations
/// 
/// USE RANDOM UUIDs (Uuid::new_v4()) FOR:
/// - ALL user-created tags
/// - ALL user-created albums  
/// - Any resource that supports polymorphic naming
/// - Any resource where multiple instances with the same name are valid
/// 
/// The semantic tagging system REQUIRES random UUIDs to support multiple
/// tags with the same name in different contexts (e.g., "Phoenix" as a city
/// vs "Phoenix" as mythology).

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_deterministic_system_tag_uuid() {
		let uuid1 = deterministic_system_tag_uuid("System");
		let uuid2 = deterministic_system_tag_uuid("System");

		// Same system tag = same UUID across all installations
		assert_eq!(uuid1, uuid2);

		// Different system tags = different UUIDs
		let uuid3 = deterministic_system_tag_uuid("Screenshot");
		assert_ne!(uuid1, uuid3);
		
		// Verify known system tags have consistent UUIDs
		use system_tags::*;
		assert_eq!(*SYSTEM_TAG_UUID, deterministic_system_tag_uuid("System"));
	}

	#[test]
	fn test_deterministic_system_album_uuid() {
		let uuid1 = deterministic_system_album_uuid("Recent Imports");
		let uuid2 = deterministic_system_album_uuid("Recent Imports");

		assert_eq!(uuid1, uuid2);
	}
}
