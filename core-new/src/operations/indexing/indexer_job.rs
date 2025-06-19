//! Indexer job implementation

use crate::{
    infrastructure::jobs::prelude::*,
    infrastructure::database::entities::{self, entry, content_identity, user_metadata, path_prefix},
    shared::types::SdPath,
    domain::content_identity::{CasGenerator, CasError},
};
use serde::{Deserialize, Serialize};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, QueryFilter, ColumnTrait};
use std::{
    collections::{HashSet, VecDeque, HashMap},
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Indexing mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IndexMode {
    /// Just filesystem metadata
    Shallow,
    /// Generate content identities
    Content,
    /// Full indexing with thumbnails and text extraction
    Deep,
}

/// Indexer job - discovers and indexes files in a location
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerJob {
    pub location_id: Uuid,
    pub root_path: SdPath,
    pub mode: IndexMode,
    
    // Resumable state
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<IndexerState>,
}

/// Indexer progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerProgress {
    pub phase: IndexPhase,
    pub current_path: String,
    pub total_found: IndexerStats,
    pub processing_rate: f32,
    pub estimated_remaining: Option<Duration>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct IndexerStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub symlinks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexPhase {
    Discovery { dirs_queued: usize },
    Processing { batch: usize, total_batches: usize },
    ContentIdentification { current: usize, total: usize },
    Finalizing,
}

/// Resumable state
#[derive(Debug, Serialize, Deserialize)]
struct IndexerState {
    phase: Phase,
    #[serde(skip, default = "Instant::now")]
    started_at: Instant,
    
    // Discovery phase
    dirs_to_walk: VecDeque<PathBuf>,
    pending_entries: Vec<DirEntry>,
    seen_paths: HashSet<PathBuf>,
    
    // Processing phase  
    entry_batches: Vec<Vec<DirEntry>>,
    
    // Content phase
    entries_for_content: Vec<(i32, PathBuf)>, // (entry_id, path)
    
    // Database operations
    path_prefix_cache: HashMap<String, i32>, // prefix -> prefix_id
    
    // Statistics
    stats: IndexerStats,
    errors: Vec<IndexError>,
    
    // Performance tracking
    #[serde(skip, default = "Instant::now")]
    last_progress_time: Instant,
    items_since_last_update: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Phase {
    Discovery,
    Processing,
    ContentIdentification,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirEntry {
    path: PathBuf,
    kind: EntryKind,
    size: u64,
    modified: Option<std::time::SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum EntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexError {
    ReadDir { path: String, error: String },
    CreateEntry { path: String, error: String },
    ContentId { path: String, error: String },
}

impl Job for IndexerJob {
    const NAME: &'static str = "indexer";
    const RESUMABLE: bool = true;
    const DESCRIPTION: Option<&'static str> = Some("Index files in a location");
}

impl JobProgress for IndexerProgress {}

#[async_trait::async_trait]
impl JobHandler for IndexerJob {
    type Output = IndexerOutput;
    
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        // Initialize or restore state
        let state = match &mut self.state {
            Some(state) => {
                ctx.log("Resuming indexer from saved state");
                state
            }
            None => {
                ctx.log("Starting new indexer job");
                self.state = Some(IndexerState::new(&self.root_path));
                self.state.as_mut().unwrap()
            }
        };
        
        // Main state machine loop
        loop {
            ctx.check_interrupt().await?;
            
            match state.phase.clone() {
                Phase::Discovery => {
                    Self::run_discovery_phase(state, &ctx, &self.root_path).await?;
                }
                Phase::Processing => {
                    let mode = self.mode; // Extract mode to avoid borrow issues
                    let location_id = self.location_id;
                    Self::run_processing_phase_impl(location_id, state, &ctx, mode).await?;
                }
                Phase::ContentIdentification => {
                    if self.mode >= IndexMode::Content {
                        Self::run_content_phase(state, &ctx).await?;
                    } else {
                        state.phase = Phase::Complete;
                    }
                }
                Phase::Complete => break,
            }
            
            // Checkpoint after each phase
            ctx.checkpoint().await?;
        }
        
        // Generate final output
        Ok(IndexerOutput {
            location_id: self.location_id,
            stats: state.stats,
            duration: state.started_at.elapsed(),
            errors: state.errors.clone(),
        })
    }
    
    async fn on_resume(&mut self, ctx: &JobContext<'_>) -> JobResult {
        // State is already loaded from serialization
        if let Some(state) = &self.state {
            ctx.log(format!("Resuming indexer in {:?} phase", state.phase));
            ctx.log(format!("Found {} files, {} dirs so far", 
                state.stats.files, state.stats.dirs));
        }
        Ok(())
    }
}

impl IndexerJob {
    /// Create a new indexer job
    pub fn new(location_id: Uuid, root_path: SdPath, mode: IndexMode) -> Self {
        Self {
            location_id,
            root_path,
            mode,
            state: None,
        }
    }
    
    /// Phase 1: Walk directories and collect entries
    async fn run_discovery_phase(state: &mut IndexerState, ctx: &JobContext<'_>, root_path: &SdPath) -> Result<(), JobError> {
        while let Some(dir_path) = state.dirs_to_walk.pop_front() {
            ctx.check_interrupt().await?;
            
            // Skip if already seen (handles symlink loops)
            if !state.seen_paths.insert(dir_path.clone()) {
                continue;
            }
            
            // Update progress
            ctx.progress(Progress::structured(IndexerProgress {
                phase: IndexPhase::Discovery { 
                    dirs_queued: state.dirs_to_walk.len() 
                },
                current_path: dir_path.to_string_lossy().to_string(),
                total_found: state.stats,
                processing_rate: state.calculate_rate(),
                estimated_remaining: None,
            }));
            
            // Read directory entries
            match Self::read_directory(&dir_path).await {
                Ok(entries) => {
                    for entry in entries {
                        match entry.kind {
                            EntryKind::Directory => {
                                state.dirs_to_walk.push_back(entry.path.clone());
                                state.stats.dirs += 1;
                            }
                            EntryKind::File => {
                                state.stats.bytes += entry.size;
                                state.stats.files += 1;
                                state.pending_entries.push(entry);
                            }
                            EntryKind::Symlink => {
                                state.stats.symlinks += 1;
                            }
                        }
                    }
                    
                    // Batch entries
                    if state.pending_entries.len() >= 1000 {
                        state.entry_batches.push(
                            std::mem::take(&mut state.pending_entries)
                        );
                    }
                }
                Err(e) => {
                    ctx.add_non_critical_error(format!("Failed to read {}: {}", dir_path.display(), e));
                    state.errors.push(IndexError::ReadDir { 
                        path: dir_path.to_string_lossy().to_string(), 
                        error: e.to_string() 
                    });
                }
            }
            
            // Update rate tracking
            state.items_since_last_update += 1;
            
            // Periodic checkpoint
            if state.stats.files % 5000 == 0 {
                ctx.checkpoint_with_state(state).await?;
            }
        }
        
        // Final batch
        if !state.pending_entries.is_empty() {
            state.entry_batches.push(
                std::mem::take(&mut state.pending_entries)
            );
        }
        
        state.phase = Phase::Processing;
        Ok(())
    }
    
    /// Phase 2: Process entry batches (implementation)
    async fn run_processing_phase_impl(location_id: Uuid, state: &mut IndexerState, ctx: &JobContext<'_>, mode: IndexMode) -> Result<(), JobError> {
        let total_batches = state.entry_batches.len();
        
        while let Some(batch) = state.entry_batches.pop() {
            ctx.check_interrupt().await?;
            
            let batch_num = total_batches - state.entry_batches.len();
            ctx.progress(Progress::structured(IndexerProgress {
                phase: IndexPhase::Processing { 
                    batch: batch_num, 
                    total_batches 
                },
                current_path: format!("Batch {}/{}", batch_num, total_batches),
                total_found: state.stats,
                processing_rate: state.calculate_rate(),
                estimated_remaining: None,
            }));
            
            // Process batch - create database entries
            for entry in batch {
                // We need the device_id from the location - for now use a placeholder
                // TODO: Get actual device_id from location/root_path
                let device_id = 1; // Placeholder - should come from location or be passed in
                
                // Use the actual location_id from the job (convert to i32 for database)
                let location_id_i32 = 1i32; // TODO: proper conversion from Uuid to i32
                
                match Self::create_entry(state, ctx, &entry, location_id_i32, device_id).await {
                    Ok(entry_id) => {
                        ctx.log(format!("Created entry {}: {}", entry_id, entry.path.display()));
                        
                        // Track for content identification if needed
                        if mode >= IndexMode::Content && entry.kind == EntryKind::File {
                            state.entries_for_content.push((entry_id, entry.path));
                        }
                    }
                    Err(e) => {
                        ctx.add_non_critical_error(format!("Failed to create entry for {}: {}", entry.path.display(), e));
                        state.errors.push(IndexError::CreateEntry { 
                            path: entry.path.to_string_lossy().to_string(), 
                            error: e.to_string() 
                        });
                    }
                }
            }
            
            ctx.checkpoint_with_state(state).await?;
        }
        
        state.phase = Phase::ContentIdentification;
        Ok(())
    }
    
    /// Phase 3: Generate content identities
    async fn run_content_phase(state: &mut IndexerState, ctx: &JobContext<'_>) -> Result<(), JobError> {
        let total = state.entries_for_content.len();
        let mut processed = 0;
        
        // Process in chunks
        while !state.entries_for_content.is_empty() {
            ctx.check_interrupt().await?;
            
            let chunk_size = 100.min(state.entries_for_content.len());
            let chunk: Vec<_> = state.entries_for_content.drain(..chunk_size).collect();
            
            processed += chunk.len();
            
            ctx.progress(Progress::structured(IndexerProgress {
                phase: IndexPhase::ContentIdentification { 
                    current: processed, 
                    total 
                },
                current_path: "Generating content identities".to_string(),
                total_found: state.stats,
                processing_rate: state.calculate_rate(),
                estimated_remaining: None,
            }));
            
            // Generate CAS IDs for content identification
            for (entry_id, path) in &chunk {
                match CasGenerator::generate_cas_id(path).await {
                    Ok(cas_id) => {
                        match Self::create_content_identity(ctx, *entry_id, path, cas_id.clone()).await {
                            Ok(()) => {
                                ctx.log(format!("Created content identity for {}: {}", path.display(), cas_id));
                            }
                            Err(e) => {
                                ctx.add_non_critical_error(format!("Failed to create content identity for {}: {}", path.display(), e));
                                state.errors.push(IndexError::ContentId { 
                                    path: path.to_string_lossy().to_string(), 
                                    error: e.to_string() 
                                });
                            }
                        }
                    }
                    Err(e) => {
                        ctx.add_non_critical_error(format!("Failed to generate CAS ID for {}: {}", path.display(), e));
                        state.errors.push(IndexError::ContentId { 
                            path: path.to_string_lossy().to_string(), 
                            error: e.to_string() 
                        });
                    }
                }
            }
            
            // Periodic checkpoint
            if processed % 1000 == 0 {
                ctx.checkpoint_with_state(state).await?;
            }
        }
        
        state.phase = Phase::Complete;
        Ok(())
    }
    
    /// Read a directory
    async fn read_directory(path: &PathBuf) -> Result<Vec<DirEntry>, std::io::Error> {
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(path).await?;
        
        while let Some(entry) = dir.next_entry().await? {
            let metadata = entry.metadata().await?;
            let kind = if metadata.is_dir() {
                EntryKind::Directory
            } else if metadata.is_symlink() {
                EntryKind::Symlink
            } else {
                EntryKind::File
            };
            
            entries.push(DirEntry {
                path: entry.path(),
                kind,
                size: metadata.len(),
                modified: metadata.modified().ok(),
            });
        }
        
        Ok(entries)
    }
    
    /// Get or create a path prefix for efficient storage
    async fn get_or_create_path_prefix(
        state: &mut IndexerState,
        ctx: &JobContext<'_>,
        device_id: i32,
        full_path: &PathBuf,
    ) -> Result<(i32, String), JobError> {
        // Find the longest common prefix
        let path_str = full_path.to_string_lossy().to_string();
        let parent = full_path.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string());
        
        // Check cache first
        if let Some(&prefix_id) = state.path_prefix_cache.get(&parent) {
            let relative = if path_str.starts_with(&parent) {
                path_str[parent.len()..].trim_start_matches('/').to_string()
            } else {
                path_str
            };
            return Ok((prefix_id, relative));
        }
        
        // Look up existing prefix in database
        let existing = entities::path_prefix::Entity::find()
            .filter(entities::path_prefix::Column::DeviceId.eq(device_id))
            .filter(entities::path_prefix::Column::Prefix.eq(&parent))
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to query path prefix: {}", e)))?;
        
        let prefix_id = if let Some(existing) = existing {
            // Use existing prefix
            existing.id
        } else {
            // Create new prefix
            let new_prefix = entities::path_prefix::ActiveModel {
                device_id: Set(device_id),
                prefix: Set(parent.clone()),
                created_at: Set(chrono::Utc::now()),
                ..Default::default()
            };
            
            let result = new_prefix.insert(ctx.library_db()).await
                .map_err(|e| JobError::execution(format!("Failed to create path prefix: {}", e)))?;
            
            result.id
        };
        
        // Cache for future use
        state.path_prefix_cache.insert(parent.clone(), prefix_id);
        
        // Calculate relative path
        let relative = if path_str.starts_with(&parent) {
            path_str[parent.len()..].trim_start_matches('/').to_string()
        } else {
            path_str
        };
        
        Ok((prefix_id, relative))
    }
    
    /// Create a user metadata record (always required)
    async fn create_user_metadata(ctx: &JobContext<'_>) -> Result<i32, JobError> {
        let metadata = entities::user_metadata::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            notes: Set(None),
            favorite: Set(false),
            hidden: Set(false),
            custom_data: Set(serde_json::json!({})),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
            ..Default::default()
        };
        
        let result = metadata.insert(ctx.library_db()).await
            .map_err(|e| JobError::execution(format!("Failed to create user metadata: {}", e)))?;
        
        Ok(result.id)
    }
    
    /// Create an entry record in the database
    async fn create_entry(
        state: &mut IndexerState,
        ctx: &JobContext<'_>,
        entry: &DirEntry,
        location_id: i32,
        device_id: i32,
    ) -> Result<i32, JobError> {
        // Get path prefix
        let (prefix_id, relative_path) = Self::get_or_create_path_prefix(
            state, ctx, device_id, &entry.path
        ).await?;
        
        // Create user metadata (always required)
        let metadata_id = Self::create_user_metadata(ctx).await?;
        
        // Get file name
        let name = entry.path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        // Convert timestamps
        let modified_at = entry.modified
            .and_then(|t| chrono::DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64, 0
            ))
            .unwrap_or_else(|| chrono::Utc::now());
        
        // Create entry
        let new_entry = entities::entry::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            prefix_id: Set(prefix_id),
            relative_path: Set(relative_path),
            name: Set(name),
            kind: Set(match entry.kind {
                EntryKind::File => "file".to_string(),
                EntryKind::Directory => "directory".to_string(),
                EntryKind::Symlink => "symlink".to_string(),
            }),
            metadata_id: Set(metadata_id),
            content_id: Set(None), // Will be set later if content indexing is enabled
            location_id: Set(Some(location_id)),
            parent_id: Set(None), // TODO: Could implement parent relationship
            size: Set(entry.size as i64),
            created_at: Set(chrono::Utc::now()),
            modified_at: Set(modified_at),
            accessed_at: Set(None),
            permissions: Set(None), // TODO: Could extract from metadata
            ..Default::default()
        };
        
        let result = new_entry.insert(ctx.library_db()).await
            .map_err(|e| JobError::execution(format!("Failed to create entry: {}", e)))?;
        
        Ok(result.id)
    }
    
    /// Create or find content identity and link to entry
    async fn create_content_identity(
        ctx: &JobContext<'_>,
        entry_id: i32,
        path: &PathBuf,
        cas_id: String,
    ) -> Result<(), JobError> {
        // Check if content identity already exists
        let existing = entities::content_identity::Entity::find()
            .filter(entities::content_identity::Column::CasId.eq(&cas_id))
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to query content identity: {}", e)))?;
        
        let content_id = if let Some(existing) = existing {
            // Update entry count and last verification
            let existing_id = existing.id;
            let mut existing_active: entities::content_identity::ActiveModel = existing.into();
            existing_active.entry_count = Set(existing_active.entry_count.unwrap() + 1);
            existing_active.last_verified_at = Set(chrono::Utc::now());
            
            existing_active.update(ctx.library_db()).await
                .map_err(|e| JobError::execution(format!("Failed to update content identity: {}", e)))?;
            
            existing_id
        } else {
            // Create new content identity
            let file_size = tokio::fs::metadata(path).await
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            
            let new_content = entities::content_identity::ActiveModel {
                uuid: Set(Uuid::new_v4()),
                full_hash: Set(None), // Could implement full hash later
                cas_id: Set(cas_id),
                cas_version: Set(1), // CAS version
                mime_type: Set(None), // TODO: Detect MIME type
                kind: Set("file".to_string()), // TODO: Detect content kind
                media_data: Set(None), // TODO: Extract media metadata
                text_content: Set(None), // TODO: Extract text content for indexing
                total_size: Set(file_size),
                entry_count: Set(1),
                first_seen_at: Set(chrono::Utc::now()),
                last_verified_at: Set(chrono::Utc::now()),
                ..Default::default()
            };
            
            let result = new_content.insert(ctx.library_db()).await
                .map_err(|e| JobError::execution(format!("Failed to create content identity: {}", e)))?;
            
            result.id
        };
        
        // Update entry to link to content identity
        let entry = entities::entry::Entity::find_by_id(entry_id)
            .one(ctx.library_db())
            .await
            .map_err(|e| JobError::execution(format!("Failed to find entry: {}", e)))?
            .ok_or_else(|| JobError::execution("Entry not found after creation".to_string()))?;
        
        let mut entry_active: entities::entry::ActiveModel = entry.into();
        entry_active.content_id = Set(Some(content_id));
        
        entry_active.update(ctx.library_db()).await
            .map_err(|e| JobError::execution(format!("Failed to link content identity to entry: {}", e)))?;
        
        Ok(())
    }
    
}

impl IndexerState {
    fn new(root_path: &SdPath) -> Self {
        let mut dirs_to_walk = VecDeque::new();
        if let Some(path) = root_path.as_local_path() {
            dirs_to_walk.push_back(path.to_path_buf());
        }
        
        Self {
            phase: Phase::Discovery,
            started_at: Instant::now(),
            dirs_to_walk,
            pending_entries: Vec::new(),
            seen_paths: HashSet::new(),
            entry_batches: Vec::new(),
            entries_for_content: Vec::new(),
            path_prefix_cache: HashMap::new(),
            stats: Default::default(),
            errors: Vec::new(),
            last_progress_time: Instant::now(),
            items_since_last_update: 0,
        }
    }
    
    fn calculate_rate(&mut self) -> f32 {
        let elapsed = self.last_progress_time.elapsed();
        if elapsed.as_secs() > 0 {
            let rate = self.items_since_last_update as f32 / elapsed.as_secs_f32();
            self.last_progress_time = Instant::now();
            self.items_since_last_update = 0;
            rate
        } else {
            0.0
        }
    }
}

/// Job output
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerOutput {
    pub location_id: Uuid,
    pub stats: IndexerStats,
    pub duration: Duration,
    pub errors: Vec<IndexError>,
}

impl From<IndexerOutput> for JobOutput {
    fn from(output: IndexerOutput) -> Self {
        JobOutput::Indexed {
            total_files: output.stats.files,
            total_dirs: output.stats.dirs,
            total_bytes: output.stats.bytes,
        }
    }
}