use sea_orm::{
    prelude::*, DatabaseConnection, QueryFilter, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::infra::database::entities::{sidecar, entry, content_identity};

/// Represents a Live Photo pair (image + video sidecar)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePhotoPair {
    /// The image entry
    pub image_entry_id: i32,
    pub image_entry_uuid: Option<Uuid>,
    pub image_content_uuid: Uuid,

    /// The video sidecar
    pub video_sidecar_path: String,
    pub video_sidecar_size: i64,
}

/// Query service for Live Photos
pub struct LivePhotoQuery;

impl LivePhotoQuery {
    /// Find all Live Photo pairs in the library
    /// This queries for all live_photo_video sidecars and their associated image entries
    pub async fn find_all_pairs(db: &DatabaseConnection) -> Result<Vec<LivePhotoPair>, DbErr> {
        // Find all Live Photo video sidecars
        let live_photo_sidecars = sidecar::Entity::find()
            .filter(sidecar::Column::Kind.eq("live_photo_video"))
            .filter(sidecar::Column::Status.eq("ready"))
            .all(db)
            .await?;

        let mut pairs = Vec::new();

        for sidecar in live_photo_sidecars {
            // Find the image entry via content identity
            let content = content_identity::Entity::find()
                .filter(content_identity::Column::Uuid.eq(sidecar.content_uuid))
                .one(db)
                .await?;

            if let Some(content) = content {
                // Find the entry for this content
                let entry = entry::Entity::find()
                    .filter(entry::Column::ContentId.eq(content.id))
                    .one(db)
                    .await?;

                if let Some(entry) = entry {
                    pairs.push(LivePhotoPair {
                        image_entry_id: entry.id,
                        image_entry_uuid: entry.uuid,
                        image_content_uuid: sidecar.content_uuid,
                        video_sidecar_path: sidecar.rel_path,
                        video_sidecar_size: sidecar.size,
                    });
                }
            }
        }

        Ok(pairs)
    }

    /// Find Live Photo pair for a specific image entry
    pub async fn find_by_entry_id(
        db: &DatabaseConnection,
        entry_id: i32,
    ) -> Result<Option<LivePhotoPair>, DbErr> {
        // Get the entry and its content UUID
        let entry = entry::Entity::find_by_id(entry_id)
            .one(db)
            .await?;

        if let Some(entry) = entry {
            if let Some(content_id) = entry.content_id {
                // Get the content identity to find its UUID
                let content = content_identity::Entity::find_by_id(content_id)
                    .one(db)
                    .await?;

                if let Some(content) = content {
                    if let Some(content_uuid) = content.uuid {
                        // Check if there's a Live Photo video sidecar for this content
                        let sidecar = sidecar::Entity::find()
                            .filter(sidecar::Column::ContentUuid.eq(content_uuid))
                            .filter(sidecar::Column::Kind.eq("live_photo_video"))
                            .filter(sidecar::Column::Status.eq("ready"))
                            .one(db)
                            .await?;

                        if let Some(sidecar) = sidecar {
                            return Ok(Some(LivePhotoPair {
                                image_entry_id: entry.id,
                                image_entry_uuid: entry.uuid,
                                image_content_uuid: content_uuid,
                                video_sidecar_path: sidecar.rel_path,
                                video_sidecar_size: sidecar.size,
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Check if an entry is part of a Live Photo (as the image component)
    pub async fn is_live_photo(
        db: &DatabaseConnection,
        entry_id: i32,
    ) -> Result<bool, DbErr> {
        Ok(Self::find_by_entry_id(db, entry_id).await?.is_some())
    }
}