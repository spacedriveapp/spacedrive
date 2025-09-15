//! Tag Usage Pattern entity
//!
//! SeaORM entity for tracking co-occurrence patterns between tags

use sea_orm::entity::prelude::*;
use sea_orm::{Set, NotSet};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tag_usage_patterns")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub tag_id: i32,
    pub co_occurrence_tag_id: i32,
    pub occurrence_count: i32,
    pub last_used_together: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::semantic_tag::Entity",
        from = "Column::TagId",
        to = "super::semantic_tag::Column::Id"
    )]
    Tag,

    #[sea_orm(
        belongs_to = "super::semantic_tag::Entity",
        from = "Column::CoOccurrenceTagId",
        to = "super::semantic_tag::Column::Id"
    )]
    CoOccurrenceTag,
}

impl Related<super::semantic_tag::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tag.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    fn new() -> Self {
        Self {
            occurrence_count: Set(1),
            last_used_together: Set(chrono::Utc::now()),
            ..ActiveModelTrait::default()
        }
    }
}

impl Model {
    /// Increment the occurrence count and update last used time
    pub fn increment_usage(&mut self) {
        self.occurrence_count += 1;
        self.last_used_together = chrono::Utc::now();
    }

    /// Check if this pattern is frequently used (threshold: 5+ occurrences)
    pub fn is_frequent(&self) -> bool {
        self.occurrence_count >= 5
    }

    /// Check if this pattern is very frequent (threshold: 20+ occurrences)
    pub fn is_very_frequent(&self) -> bool {
        self.occurrence_count >= 20
    }

    /// Get the usage frequency as a score (higher = more frequent)
    pub fn frequency_score(&self) -> f32 {
        (self.occurrence_count as f32).ln().max(0.0)
    }

    /// Check if this pattern was used recently (within 30 days)
    pub fn is_recent(&self) -> bool {
        let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
        self.last_used_together > thirty_days_ago
    }

    /// Calculate relevance score based on frequency and recency
    pub fn relevance_score(&self) -> f32 {
        let frequency_weight = self.frequency_score() * 0.7;
        let recency_weight = if self.is_recent() { 0.3 } else { 0.1 };

        frequency_weight + recency_weight
    }
}