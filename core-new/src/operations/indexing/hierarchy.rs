//! Hierarchical query helpers using closure table

use crate::infrastructure::database::entities::{entry, entry_closure};
use sea_orm::{
    ColumnTrait, DbConn, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    JoinType, RelationTrait, Condition,
};
use std::path::PathBuf;

/// Hierarchical query helpers for efficient tree operations
pub struct HierarchyQuery;

impl HierarchyQuery {
    /// Get direct children of an entry
    pub async fn get_children(
        db: &DbConn,
        parent_id: i32,
    ) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
        entry::Entity::find()
            .filter(entry::Column::ParentId.eq(parent_id))
            .order_by_asc(entry::Column::Name)
            .all(db)
            .await
    }

    /// Get all descendants of an entry (recursive)
    pub async fn get_descendants(
        db: &DbConn,
        ancestor_id: i32,
    ) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
        entry::Entity::find()
            .join(
                JoinType::InnerJoin,
                entry_closure::Entity,
                Condition::all()
                    .add(entry_closure::Column::DescendantId.eq(entry::Column::Id))
                    .add(entry_closure::Column::AncestorId.eq(ancestor_id))
                    .add(entry_closure::Column::Depth.gt(0)),
            )
            .order_by_asc(entry_closure::Column::Depth)
            .order_by_asc(entry::Column::Name)
            .all(db)
            .await
    }

    /// Get all ancestors of an entry (path to root)
    pub async fn get_ancestors(
        db: &DbConn,
        descendant_id: i32,
    ) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
        entry::Entity::find()
            .join(
                JoinType::InnerJoin,
                entry_closure::Entity,
                Condition::all()
                    .add(entry_closure::Column::AncestorId.eq(entry::Column::Id))
                    .add(entry_closure::Column::DescendantId.eq(descendant_id))
                    .add(entry_closure::Column::Depth.gt(0)),
            )
            .order_by_desc(entry_closure::Column::Depth)
            .all(db)
            .await
    }

    /// Get entries at a specific depth below an ancestor
    pub async fn get_at_depth(
        db: &DbConn,
        ancestor_id: i32,
        depth: i32,
    ) -> Result<Vec<entry::Model>, sea_orm::DbErr> {
        entry::Entity::find()
            .join(
                JoinType::InnerJoin,
                entry_closure::Entity,
                Condition::all()
                    .add(entry_closure::Column::DescendantId.eq(entry::Column::Id))
                    .add(entry_closure::Column::AncestorId.eq(ancestor_id))
                    .add(entry_closure::Column::Depth.eq(depth)),
            )
            .order_by_asc(entry::Column::Name)
            .all(db)
            .await
    }

    /// Build a full path for an entry by traversing ancestors
    pub async fn build_full_path(
        db: &DbConn,
        entry_id: i32,
        location_path: &str,
    ) -> Result<PathBuf, sea_orm::DbErr> {
        // Get the entry itself
        let entry = entry::Entity::find_by_id(entry_id)
            .one(db)
            .await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound("Entry not found".to_string()))?;

        // Get all ancestors in order (root to parent)
        let ancestors = Self::get_ancestors(db, entry_id).await?;

        // Build the path
        let mut path = PathBuf::from(location_path);
        
        // Add ancestor names
        for ancestor in ancestors {
            path.push(&ancestor.name);
        }
        
        // Add the entry's own name
        path.push(&entry.name);

        Ok(path)
    }

    /// Count total descendants of an entry
    pub async fn count_descendants(
        db: &DbConn,
        ancestor_id: i32,
    ) -> Result<u64, sea_orm::DbErr> {
        entry_closure::Entity::find()
            .filter(entry_closure::Column::AncestorId.eq(ancestor_id))
            .filter(entry_closure::Column::Depth.gt(0))
            .count(db)
            .await
    }

    /// Get subtree size (total size of all descendant files)
    pub async fn get_subtree_size(
        db: &DbConn,
        ancestor_id: i32,
    ) -> Result<i64, sea_orm::DbErr> {
        let descendants = Self::get_descendants(db, ancestor_id).await?;
        Ok(descendants.iter().map(|e| e.size).sum())
    }

    /// Check if an entry is an ancestor of another
    pub async fn is_ancestor_of(
        db: &DbConn,
        potential_ancestor_id: i32,
        potential_descendant_id: i32,
    ) -> Result<bool, sea_orm::DbErr> {
        let count = entry_closure::Entity::find()
            .filter(entry_closure::Column::AncestorId.eq(potential_ancestor_id))
            .filter(entry_closure::Column::DescendantId.eq(potential_descendant_id))
            .filter(entry_closure::Column::Depth.gt(0))
            .count(db)
            .await?;
        
        Ok(count > 0)
    }

    /// Find common ancestor of two entries
    pub async fn find_common_ancestor(
        db: &DbConn,
        entry1_id: i32,
        entry2_id: i32,
    ) -> Result<Option<entry::Model>, sea_orm::DbErr> {
        // Get ancestors of both entries
        let ancestors1 = Self::get_ancestors(db, entry1_id).await?;
        let ancestors2 = Self::get_ancestors(db, entry2_id).await?;

        // Find the first common ancestor (starting from the deepest)
        for ancestor1 in ancestors1.iter().rev() {
            for ancestor2 in &ancestors2 {
                if ancestor1.id == ancestor2.id {
                    return Ok(Some(ancestor1.clone()));
                }
            }
        }

        Ok(None)
    }
}