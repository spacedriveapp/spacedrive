//! Directory size aggregation phase

use crate::{
    infrastructure::{
        jobs::prelude::{JobContext, JobError, Progress},
        database::entities,
    },
    operations::indexing::{
        state::{IndexerState, IndexPhase, Phase},
    },
};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, QueryOrder, DbErr, DatabaseConnection, ActiveModelTrait, ActiveValue::Set};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

/// Run the directory aggregation phase
pub async fn run_aggregation_phase(
    location_id: Uuid,
    state: &mut IndexerState,
    ctx: &JobContext<'_>,
) -> Result<(), JobError> {
    ctx.log("Starting directory size aggregation phase");
    
    // Get the location record
    let location_record = entities::location::Entity::find()
        .filter(entities::location::Column::Uuid.eq(location_id))
        .one(ctx.library_db())
        .await
        .map_err(|e| JobError::execution(format!("Failed to find location: {}", e)))?
        .ok_or_else(|| JobError::execution("Location not found in database".to_string()))?;
    
    let location_id_i32 = location_record.id;
    
    // Find all directories in this location
    let directories = entities::entry::Entity::find()
        .filter(entities::entry::Column::LocationId.eq(location_id_i32))
        .filter(entities::entry::Column::Kind.eq(1)) // Directory
        .order_by_desc(entities::entry::Column::RelativePath) // Process deepest first
        .all(ctx.library_db())
        .await
        .map_err(|e| JobError::execution(format!("Failed to query directories: {}", e)))?;
    
    let total_dirs = directories.len();
    ctx.log(format!("Found {} directories to aggregate", total_dirs));
    
    // Build parent->children mapping
    let mut children_by_parent: HashMap<Option<i32>, Vec<i32>> = HashMap::new();
    for dir in &directories {
        children_by_parent.entry(dir.parent_id).or_default().push(dir.id);
    }
    
    // Process directories from leaves to root
    let mut processed = 0;
    let aggregator = DirectoryAggregator::new(ctx.library_db().clone());
    
    for directory in directories {
        ctx.check_interrupt().await?;
        
        processed += 1;
        ctx.progress(Progress::structured(crate::operations::indexing::IndexerProgress {
            phase: IndexPhase::Finalizing,
            current_path: format!("Aggregating directory {}/{}: {}", processed, total_dirs, directory.name),
            total_found: state.stats,
            processing_rate: state.calculate_rate(),
            estimated_remaining: state.estimate_remaining(),
        }));
        
        // Calculate aggregate values for this directory
        match aggregator.aggregate_directory(&directory).await {
            Ok((aggregate_size, child_count, file_count)) => {
                // Update the directory entry
                let directory_name = directory.name.clone();
                let mut active_dir: entities::entry::ActiveModel = directory.into();
                active_dir.aggregate_size = Set(aggregate_size);
                active_dir.child_count = Set(child_count);
                active_dir.file_count = Set(file_count);
                
                active_dir.update(ctx.library_db()).await
                    .map_err(|e| JobError::execution(format!("Failed to update directory aggregates: {}", e)))?;
                
                ctx.log(format!("✅ Aggregated {}: {} bytes, {} children, {} files", 
                    directory_name, aggregate_size, child_count, file_count));
            }
            Err(e) => {
                ctx.add_non_critical_error(format!("Failed to aggregate directory {}: {}", directory.name, e));
            }
        }
        
        // Checkpoint periodically
        if processed % 100 == 0 {
            ctx.checkpoint_with_state(state).await?;
        }
    }
    
    ctx.log(format!("Directory aggregation complete: {} directories processed", processed));
    state.phase = Phase::ContentIdentification;
    Ok(())
}

struct DirectoryAggregator {
    db: DatabaseConnection,
}

impl DirectoryAggregator {
    fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
    
    /// Calculate aggregate size, child count, and file count for a directory
    async fn aggregate_directory(&self, directory: &entities::entry::Model) -> Result<(i64, i32, i32), DbErr> {
        // Get all direct children
        let children = entities::entry::Entity::find()
            .filter(entities::entry::Column::ParentId.eq(directory.id))
            .all(&self.db)
            .await?;
        
        let mut aggregate_size = 0i64;
        let child_count = children.len() as i32;
        let mut file_count = 0i32;
        
        for child in children {
            match child.kind {
                0 => { // File
                    aggregate_size += child.size;
                    file_count += 1;
                }
                1 => { // Directory
                    aggregate_size += child.aggregate_size;
                    file_count += child.file_count;
                }
                2 => { // Symlink - count as file
                    aggregate_size += child.size;
                    file_count += 1;
                }
                _ => {} // Unknown type, skip
            }
        }
        
        Ok((aggregate_size, child_count, file_count))
    }
}

/// One-time migration to calculate all directory sizes for existing data
pub async fn migrate_directory_sizes(db: &DatabaseConnection) -> Result<(), DbErr> {
    // Get all locations
    let locations = entities::location::Entity::find().all(db).await?;
    
    for location in locations {
        tracing::info!("Migrating directory sizes for location: {}", location.name.as_deref().unwrap_or("Unknown"));
        
        // Find all directories in this location, ordered by path depth (deepest first)
        let directories = entities::entry::Entity::find()
            .filter(entities::entry::Column::LocationId.eq(location.id))
            .filter(entities::entry::Column::Kind.eq(1)) // Directory
            .order_by_desc(entities::entry::Column::RelativePath)
            .all(db)
            .await?;
        
        let aggregator = DirectoryAggregator::new(db.clone());
        
        for directory in directories {
            match aggregator.aggregate_directory(&directory).await {
                Ok((aggregate_size, child_count, file_count)) => {
                    let mut active_dir: entities::entry::ActiveModel = directory.into();
                    active_dir.aggregate_size = Set(aggregate_size);
                    active_dir.child_count = Set(child_count);
                    active_dir.file_count = Set(file_count);
                    
                    active_dir.update(db).await?;
                }
                Err(e) => {
                    tracing::warn!("Failed to aggregate directory {}: {}", directory.name, e);
                }
            }
        }
    }
    
    Ok(())
}