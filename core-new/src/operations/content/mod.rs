//! Content operations for library-scoped content management

pub mod action;

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::infrastructure::database::entities::{
	content_identity::{self, Entity as ContentIdentity, Model as ContentIdentityModel},
	entry::{self, Entity as Entry, Model as EntryModel},
};

pub use action::ContentAction;
use crate::shared::errors::Result;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentInstance {
	pub entry_uuid: Option<Uuid>,
	pub path: String, // TODO: Replace with SdPath when available
	pub device_uuid: Uuid,
	pub size: i64,
	pub modified_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryContentStats {
	pub entry_count: i32,
	pub total_size: i64,    // Size of one instance
	pub combined_size: i64, // Calculated on-demand (entry_count * total_size)
	pub integrity_hash: Option<String>,
	pub content_hash: String,
	pub mime_type_id: Option<i32>,
	pub kind_id: i32,
	pub has_media_data: bool,
	pub first_seen: DateTime<Utc>,
	pub last_verified: DateTime<Utc>,
}

pub struct ContentService {
	library_db: Arc<DatabaseConnection>,
}

impl ContentService {
	pub fn new(library_db: Arc<DatabaseConnection>) -> Self {
		Self { library_db }
	}

	/// Find all instances of content within this library only
	pub async fn find_content_instances(
		&self,
		content_identity_uuid: Uuid,
	) -> Result<Vec<ContentInstance>> {
		// First find the content identity by UUID
		let content_identity = ContentIdentity::find()
			.filter(content_identity::Column::Uuid.eq(content_identity_uuid))
			.one(&*self.library_db)
			.await?
			.ok_or_else(|| {
				crate::shared::errors::CoreError::NotFound("Content identity not found".to_string())
			})?;

		// Find all entries that reference this content identity
		let entries = Entry::find()
			.filter(entry::Column::ContentId.eq(content_identity.id))
			.all(&*self.library_db)
			.await?;

		let mut instances = Vec::new();
		for entry in entries {
			// TODO: Replace with proper SdPath materialization once available
			let path = format!("{}/{}", entry.relative_path, entry.name);

			// TODO: Get device UUID from location when that relationship is available
			let device_uuid = Uuid::new_v4(); // Placeholder

			instances.push(ContentInstance {
				entry_uuid: entry.uuid,
				path,
				device_uuid,
				size: entry.size,
				modified_at: Some(entry.modified_at),
			});
		}

		Ok(instances)
	}

	/// Get content statistics within this library
	pub async fn get_content_stats(
		&self,
		content_identity_uuid: Uuid,
	) -> Result<LibraryContentStats> {
		let content_identity = ContentIdentity::find()
			.filter(content_identity::Column::Uuid.eq(content_identity_uuid))
			.one(&*self.library_db)
			.await?
			.ok_or_else(|| {
				crate::shared::errors::CoreError::NotFound("Content identity not found".to_string())
			})?;

		Ok(LibraryContentStats {
			entry_count: content_identity.entry_count,
			total_size: content_identity.total_size,
			combined_size: content_identity.combined_size(),
			integrity_hash: content_identity.integrity_hash,
			content_hash: content_identity.content_hash,
			mime_type_id: content_identity.mime_type_id,
			kind_id: content_identity.kind_id,
			has_media_data: content_identity.media_data.is_some(),
			first_seen: content_identity.first_seen_at,
			last_verified: content_identity.last_verified_at,
		})
	}

	/// Find content identity by content hash
	pub async fn find_by_content_hash(
		&self,
		content_hash: &str,
	) -> Result<Option<ContentIdentityModel>> {
		let content_identity = ContentIdentity::find()
			.filter(content_identity::Column::ContentHash.eq(content_hash))
			.one(&*self.library_db)
			.await?;

		Ok(content_identity)
	}

	/// Get all content identities with entry counts above threshold
	pub async fn find_duplicated_content(
		&self,
		min_instances: i32,
	) -> Result<Vec<ContentIdentityModel>> {
		let content_identities = ContentIdentity::find()
			.filter(content_identity::Column::EntryCount.gte(min_instances))
			.all(&*self.library_db)
			.await?;

		Ok(content_identities)
	}

	/// Calculate total library deduplication savings
	pub async fn calculate_deduplication_savings(&self) -> Result<i64> {
		let content_identities = ContentIdentity::find()
			.filter(content_identity::Column::EntryCount.gt(1)) // Only duplicated content
			.all(&*self.library_db)
			.await?;

		let total_savings: i64 = content_identities
			.iter()
			.map(|content| {
				// Savings = (instances - 1) * size_per_instance
				(content.entry_count - 1) as i64 * content.total_size
			})
			.sum();

		Ok(total_savings)
	}
}
