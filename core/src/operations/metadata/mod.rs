//! Metadata operations for hierarchical metadata management

pub mod action;

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::infrastructure::database::entities::{
	content_identity::{self, Entity as ContentIdentity, Model as ContentIdentityModel},
	entry::{self, Entity as Entry, Model as EntryModel},
	tag::{self, Entity as Tag, Model as TagModel},
	user_metadata::{
		self, ActiveModel as UserMetadataActiveModel, Entity as UserMetadata,
		Model as UserMetadataModel,
	},
	user_metadata_tag::{
		self, ActiveModel as UserMetadataTagActiveModel, Entity as UserMetadataTag,
	},
};

pub use action::MetadataAction;
use crate::shared::errors::Result;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataTarget {
	/// Metadata for this specific file instance (syncs with Index domain)
	Entry(Uuid),
	/// Metadata for all instances of this content within library (syncs with UserMetadata domain)
	Content(Uuid),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetadataScope {
	Entry,   // File-specific (higher priority)
	Content, // Content-universal (lower priority)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetadataDisplay {
	pub notes: Vec<MetadataNote>, // Both entry and content notes shown
	pub tags: Vec<MetadataTag>,   // Both entry and content tags shown
	pub favorite: bool,           // Entry-level overrides content-level
	pub hidden: bool,             // Entry-level overrides content-level
	pub custom_data: Option<serde_json::Value>, // Entry-level overrides content-level
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetadataNote {
	pub content: String,
	pub scope: MetadataScope,
	pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetadataTag {
	pub tag: TagModel,
	pub scope: MetadataScope,
	pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetadataUpdate {
	pub notes: Option<String>,
	pub favorite: Option<bool>,
	pub hidden: Option<bool>,
	pub custom_data: Option<serde_json::Value>,
	pub tag_uuids: Option<Vec<Uuid>>,
}

pub struct MetadataService {
	library_db: Arc<DatabaseConnection>,
	current_device_uuid: Uuid,
}

impl MetadataService {
	pub fn new(library_db: Arc<DatabaseConnection>, current_device_uuid: Uuid) -> Self {
		Self {
			library_db,
			current_device_uuid,
		}
	}

	/// Add metadata (notes, tags, favorites) with flexible targeting
	pub async fn add_metadata(
		&self,
		target: MetadataTarget,
		metadata_update: MetadataUpdate,
	) -> Result<UserMetadataModel> {
		match target {
			MetadataTarget::Entry(entry_uuid) => {
				// File-specific metadata - create entry-scoped UserMetadata
				let user_metadata = UserMetadataActiveModel {
					uuid: Set(Uuid::new_v4()),
					entry_uuid: Set(Some(entry_uuid)),
					content_identity_uuid: Set(None), // Mutually exclusive
					notes: Set(metadata_update.notes),
					favorite: Set(metadata_update.favorite.unwrap_or(false)),
					hidden: Set(metadata_update.hidden.unwrap_or(false)),
					custom_data: Set(metadata_update.custom_data.unwrap_or_default()),
					created_at: Set(Utc::now()),
					updated_at: Set(Utc::now()),
					..Default::default()
				}
				.insert(&*self.library_db)
				.await?;

				// Add tags if provided
				if let Some(tag_uuids) = metadata_update.tag_uuids {
					self.add_tags_to_metadata(user_metadata.id, tag_uuids)
						.await?;
				}

				Ok(user_metadata)
			}

			MetadataTarget::Content(content_identity_uuid) => {
				// Content-universal metadata - create content-scoped UserMetadata
				let user_metadata = UserMetadataActiveModel {
					uuid: Set(Uuid::new_v4()),
					entry_uuid: Set(None), // Mutually exclusive
					content_identity_uuid: Set(Some(content_identity_uuid)),
					notes: Set(metadata_update.notes),
					favorite: Set(metadata_update.favorite.unwrap_or(false)),
					hidden: Set(metadata_update.hidden.unwrap_or(false)),
					custom_data: Set(metadata_update.custom_data.unwrap_or_default()),
					created_at: Set(Utc::now()),
					updated_at: Set(Utc::now()),
					..Default::default()
				}
				.insert(&*self.library_db)
				.await?;

				// Add tags if provided
				if let Some(tag_uuids) = metadata_update.tag_uuids {
					self.add_tags_to_metadata(user_metadata.id, tag_uuids)
						.await?;
				}

				Ok(user_metadata)
			}
		}
	}

	/// Get hierarchical metadata display for an entry (both entry and content metadata shown)
	pub async fn get_entry_metadata_display(&self, entry_uuid: Uuid) -> Result<MetadataDisplay> {
		let mut display = MetadataDisplay {
			notes: Vec::new(),
			tags: Vec::new(),
			favorite: false,
			hidden: false,
			custom_data: None,
		};

		// Get entry-specific metadata
		let entry_metadata = UserMetadata::find()
			.filter(user_metadata::Column::EntryUuid.eq(entry_uuid))
			.find_with_related(Tag)
			.all(&*self.library_db)
			.await?;

		for (metadata, tags) in entry_metadata {
			// Notes - show both levels
			if let Some(notes) = metadata.notes {
				display.notes.push(MetadataNote {
					content: notes,
					scope: MetadataScope::Entry,
					created_at: metadata.created_at,
				});
			}

			// Tags - show both levels
			for tag in tags {
				display.tags.push(MetadataTag {
					tag,
					scope: MetadataScope::Entry,
					created_at: metadata.created_at,
				});
			}

			// Favorites/Hidden - entry overrides (higher priority)
			display.favorite = metadata.favorite;
			display.hidden = metadata.hidden;
			display.custom_data = Some(metadata.custom_data);
		}

		// Get content-level metadata if entry has content identity
		if let Some(entry) = Entry::find()
			.filter(entry::Column::Uuid.eq(entry_uuid))
			.one(&*self.library_db)
			.await?
		{
			if let Some(content_id) = entry.content_id {
				if let Some(content_identity) = ContentIdentity::find_by_id(content_id)
					.one(&*self.library_db)
					.await?
				{
					if let Some(content_uuid) = content_identity.uuid {
						let content_metadata = UserMetadata::find()
							.filter(user_metadata::Column::ContentIdentityUuid.eq(content_uuid))
							.find_with_related(Tag)
							.all(&*self.library_db)
							.await?;

						for (metadata, tags) in content_metadata {
							// Notes - show both levels
							if let Some(notes) = metadata.notes {
								display.notes.push(MetadataNote {
									content: notes,
									scope: MetadataScope::Content,
									created_at: metadata.created_at,
								});
							}

							// Tags - show both levels
							for tag in tags {
								display.tags.push(MetadataTag {
									tag,
									scope: MetadataScope::Content,
									created_at: metadata.created_at,
								});
							}

							// Favorites/Hidden - only use if no entry-level override
							if !display.favorite && metadata.favorite {
								display.favorite = true;
							}
							if !display.hidden && metadata.hidden {
								display.hidden = true;
							}
							if display.custom_data.is_none() {
								display.custom_data = Some(metadata.custom_data);
							}
						}
					}
				}
			}
		}

		Ok(display)
	}

	/// Promote entry-level metadata to content-level ("Apply to all instances")
	pub async fn promote_to_content(
		&self,
		entry_metadata_id: i32,
		content_identity_uuid: Uuid,
	) -> Result<UserMetadataModel> {
		// Get existing entry-level metadata
		let entry_metadata = UserMetadata::find_by_id(entry_metadata_id)
			.one(&*self.library_db)
			.await?
			.ok_or_else(|| {
				crate::shared::errors::CoreError::NotFound("Metadata not found".to_string())
			})?;

		// Create new content-level metadata (entry-level remains for hierarchy)
		let content_metadata = UserMetadataActiveModel {
			uuid: Set(Uuid::new_v4()),
			entry_uuid: Set(None),
			content_identity_uuid: Set(Some(content_identity_uuid)),
			notes: Set(entry_metadata.notes.clone()),
			favorite: Set(entry_metadata.favorite),
			hidden: Set(entry_metadata.hidden),
			custom_data: Set(entry_metadata.custom_data.clone()),
			created_at: Set(Utc::now()),
			updated_at: Set(Utc::now()),
			..Default::default()
		}
		.insert(&*self.library_db)
		.await?;

		// Copy tags to new content-level metadata
		let entry_tags = UserMetadataTag::find()
			.filter(user_metadata_tag::Column::UserMetadataId.eq(entry_metadata_id))
			.all(&*self.library_db)
			.await?;

		for entry_tag in entry_tags {
			UserMetadataTagActiveModel {
				user_metadata_id: Set(content_metadata.id),
				tag_uuid: Set(entry_tag.tag_uuid),
				created_at: Set(Utc::now()),
				device_uuid: Set(self.current_device_uuid),
				..Default::default()
			}
			.insert(&*self.library_db)
			.await?;
		}

		Ok(content_metadata)
	}

	async fn add_tags_to_metadata(&self, metadata_id: i32, tag_uuids: Vec<Uuid>) -> Result<()> {
		for tag_uuid in tag_uuids {
			UserMetadataTagActiveModel {
				user_metadata_id: Set(metadata_id),
				tag_uuid: Set(tag_uuid),
				created_at: Set(Utc::now()),
				device_uuid: Set(self.current_device_uuid),
				..Default::default()
			}
			.insert(&*self.library_db)
			.await?;
		}
		Ok(())
	}
}
