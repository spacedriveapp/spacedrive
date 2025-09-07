use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    entity::prelude::*, ActiveValue, QueryFilter, QuerySelect, TransactionTrait,
};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    context::CoreContext,
    infra::db::entities::{
        sidecar::{self, Entity as Sidecar},
        sidecar_availability::{self, Entity as SidecarAvailability},
    },
    library::Library,
    ops::sidecar::{
        SidecarFormat, SidecarKind, SidecarPath, SidecarPathBuilder, SidecarStatus, SidecarVariant,
    },
};

/// Manages the Virtual Sidecar System
pub struct SidecarManager {
    context: Arc<CoreContext>,
    /// Path builders per library
    path_builders: RwLock<HashMap<Uuid, Arc<SidecarPathBuilder>>>,
    /// Active generation tasks
    active_tasks: Mutex<HashMap<(Uuid, String, String), tokio::task::JoinHandle<()>>>,
}

impl SidecarManager {
    /// Create a new sidecar manager
    pub fn new(context: Arc<CoreContext>) -> Self {
        Self {
            context,
            path_builders: RwLock::new(HashMap::new()),
            active_tasks: Mutex::new(HashMap::new()),
        }
    }

    /// Initialize path builder for a library
    pub async fn init_library(&self, library: &Library) -> Result<()> {
        let library_path = library.path();
        let sidecars_dir = library_path.join("sidecars");

        // Ensure sidecars directory exists
        tokio::fs::create_dir_all(&sidecars_dir).await?;

        // Create path builder
        let builder = Arc::new(SidecarPathBuilder::new(&library_path));

        let mut builders = self.path_builders.write().await;
        builders.insert(library.id(), builder);

        info!("Initialized sidecar manager for library {}", library.id());
        Ok(())
    }

    /// Remove path builder for a library
    pub async fn deinit_library(&self, library_id: &Uuid) {
        let mut builders = self.path_builders.write().await;
        builders.remove(library_id);

        info!("Deinitialized sidecar manager for library {}", library_id);
    }

    /// Get path builder for a library
    async fn get_path_builder(&self, library_id: &Uuid) -> Result<Arc<SidecarPathBuilder>> {
        let builders = self.path_builders.read().await;
        builders
            .get(library_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Library {} not initialized", library_id))
    }

    /// Compute sidecar path
    pub async fn compute_path(
        &self,
        library_id: &Uuid,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        format: &SidecarFormat,
    ) -> Result<SidecarPath> {
        let builder = self.get_path_builder(library_id).await?;
        Ok(builder.build(content_uuid, kind, variant, format))
    }

    /// Check if a sidecar exists in the filesystem
    pub async fn exists(
        &self,
        library_id: &Uuid,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        format: &SidecarFormat,
    ) -> Result<bool> {
        let path = self.compute_path(library_id, content_uuid, kind, variant, format).await?;
        Ok(tokio::fs::try_exists(&path.absolute_path).await?)
    }

    /// Get sidecar presence for multiple content items
    pub async fn get_presence(
        &self,
        library: &Library,
        content_uuids: &[Uuid],
        kind: &SidecarKind,
        variants: &[SidecarVariant],
    ) -> Result<HashMap<Uuid, HashMap<String, SidecarPresence>>> {
        let db = library.db();

        // Query database for local sidecars
        let sidecars = Sidecar::find()
            .filter(sidecar::Column::ContentUuid.is_in(content_uuids.to_vec()))
            .filter(sidecar::Column::Kind.eq(kind.as_str()))
            .filter(sidecar::Column::Variant.is_in(variants.iter().map(|v| v.as_str())))
            .all(db.conn())
            .await?;

        // Build presence map
        let mut presence_map: HashMap<Uuid, HashMap<String, SidecarPresence>> = HashMap::new();

        for sidecar in sidecars {
            let entry = presence_map
                .entry(sidecar.content_uuid)
                .or_insert_with(HashMap::new);

            let path = self.compute_path(
                &library.id(),
                &sidecar.content_uuid,
                &kind,
                &SidecarVariant::new(&sidecar.variant),
                &sidecar.format.as_str().try_into().map_err(|e: String| anyhow::anyhow!(e))?,
            ).await?;

            entry.insert(
                sidecar.variant.clone(),
                SidecarPresence {
                    local: true,
                    path: Some(path.relative_path),
                    status: sidecar.status.as_str().try_into().map_err(|e: String| anyhow::anyhow!(e))?,
                    devices: vec![],
                },
            );
        }

        // Query availability on other devices
        let availability = SidecarAvailability::find()
            .filter(sidecar_availability::Column::ContentUuid.is_in(content_uuids.to_vec()))
            .filter(sidecar_availability::Column::Kind.eq(kind.as_str()))
            .filter(sidecar_availability::Column::Variant.is_in(variants.iter().map(|v| v.as_str())))
            .filter(sidecar_availability::Column::Has.eq(true))
            .all(db.conn())
            .await?;

        // Add remote device availability
        for avail in availability {
            let entry = presence_map
                .entry(avail.content_uuid)
                .or_insert_with(HashMap::new)
                .entry(avail.variant.clone())
                .or_insert(SidecarPresence {
                    local: false,
                    path: None,
                    status: SidecarStatus::Pending,
                    devices: vec![],
                });

            entry.devices.push(avail.device_uuid);
        }

        Ok(presence_map)
    }

    /// Get or enqueue a sidecar
    pub async fn get_or_enqueue(
        &self,
        library: &Library,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        format: &SidecarFormat,
    ) -> Result<SidecarResult> {
        let path = self.compute_path(&library.id(), content_uuid, kind, variant, format).await?;

        // Check if it exists locally
        if tokio::fs::try_exists(&path.absolute_path).await? {
            return Ok(SidecarResult::Ready(path.relative_path));
        }

        // Check database
        let db = library.db();
        let existing = Sidecar::find()
            .filter(sidecar::Column::ContentUuid.eq(*content_uuid))
            .filter(sidecar::Column::Kind.eq(kind.as_str()))
            .filter(sidecar::Column::Variant.eq(variant.as_str()))
            .one(db.conn())
            .await?;

        if let Some(sidecar) = existing {
            match sidecar.status.as_str() {
                "ready" => Ok(SidecarResult::Ready(path.relative_path)),
                "pending" => Ok(SidecarResult::Pending),
                "failed" => {
                    // Re-enqueue failed sidecars
                    self.enqueue_generation(library, content_uuid, kind, variant, format).await?;
                    Ok(SidecarResult::Pending)
                }
                _ => Ok(SidecarResult::Pending),
            }
        } else {
            // Enqueue for generation
            self.enqueue_generation(library, content_uuid, kind, variant, format).await?;
            Ok(SidecarResult::Pending)
        }
    }

    /// Enqueue sidecar generation
    async fn enqueue_generation(
        &self,
        library: &Library,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        format: &SidecarFormat,
    ) -> Result<()> {
        let db = library.db();
        let path = self.compute_path(&library.id(), content_uuid, kind, variant, format).await?;

        // Insert pending record
        let sidecar = sidecar::ActiveModel {
            content_uuid: ActiveValue::Set(*content_uuid),
            kind: ActiveValue::Set(kind.as_str().to_string()),
            variant: ActiveValue::Set(variant.as_str().to_string()),
            format: ActiveValue::Set(format.as_str().to_string()),
            rel_path: ActiveValue::Set(path.relative_path.to_string_lossy().to_string()),
            size: ActiveValue::Set(0),
            checksum: ActiveValue::Set(None),
            status: ActiveValue::Set("pending".to_string()),
            source: ActiveValue::Set(Some("sidecar_manager".to_string())),
            version: ActiveValue::Set(1),
            created_at: ActiveValue::Set(Utc::now()),
            updated_at: ActiveValue::Set(Utc::now()),
            ..Default::default()
        };

        sidecar.insert(db.conn()).await?;

        // TODO: Dispatch to job system
        info!(
            "Enqueued sidecar generation: {} {} {} for {}",
            kind.as_str(),
            variant.as_str(),
            format.as_str(),
            content_uuid
        );

        Ok(())
    }

    /// Create a reference sidecar that links to an existing entry
    /// This allows tracking files in their original locations without moving them
    pub async fn create_reference_sidecar(
        &self,
        library: &Library,
        content_uuid: &Uuid,
        source_entry_id: i32,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        format: &SidecarFormat,
        size: u64,
        checksum: Option<String>,
    ) -> Result<()> {
        let db = library.db();

        // For reference sidecars, we use the source entry's path
        // The rel_path will be empty as the file is not in our sidecar directory
        let sidecar = sidecar::ActiveModel {
            content_uuid: ActiveValue::Set(*content_uuid),
            kind: ActiveValue::Set(kind.as_str().to_string()),
            variant: ActiveValue::Set(variant.as_str().to_string()),
            format: ActiveValue::Set(format.as_str().to_string()),
            rel_path: ActiveValue::Set("".to_string()), // Empty for reference sidecars
            source_entry_id: ActiveValue::Set(Some(source_entry_id)),
            size: ActiveValue::Set(size as i64),
            checksum: ActiveValue::Set(checksum.clone()),
            status: ActiveValue::Set("ready".to_string()),
            source: ActiveValue::Set(Some("reference".to_string())),
            version: ActiveValue::Set(1),
            created_at: ActiveValue::Set(Utc::now()),
            updated_at: ActiveValue::Set(Utc::now()),
            ..Default::default()
        };

        sidecar.insert(db.conn()).await?;

        // Update local availability
        self.update_local_availability(
            library,
            content_uuid,
            kind,
            variant,
            true,
            Some(size),
            checksum,
        ).await?;

        info!(
            "Created reference sidecar: {} {} {} for {} (entry_id: {})",
            kind.as_str(),
            variant.as_str(),
            format.as_str(),
            content_uuid,
            source_entry_id
        );

        Ok(())
    }

    /// Convert reference sidecars to owned sidecars by moving files
    pub async fn convert_reference_to_owned(
        &self,
        library: &Library,
        content_uuid: &Uuid,
    ) -> Result<()> {
        let db = library.db();

        // Find all reference sidecars for this content
        let reference_sidecars = Sidecar::find()
            .filter(sidecar::Column::ContentUuid.eq(*content_uuid))
            .filter(sidecar::Column::SourceEntryId.is_not_null())
            .all(db.conn())
            .await?;

        for sidecar in reference_sidecars {
            if let Some(source_entry_id) = sidecar.source_entry_id {
                // Get the source entry to find the file path
                use crate::infra::db::entities::entry;
                let source_entry = entry::Entity::find_by_id(source_entry_id)
                    .one(db.conn())
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Source entry not found"))?;

                // Compute the target sidecar path
                let kind = sidecar.kind.as_str().try_into().map_err(|e: String| anyhow::anyhow!(e))?;
                let variant = SidecarVariant::new(&sidecar.variant);
                let format = sidecar.format.as_str().try_into().map_err(|e: String| anyhow::anyhow!(e))?;

                let target_path = self.compute_path(
                    &library.id(),
                    content_uuid,
                    &kind,
                    &variant,
                    &format,
                ).await?;

                // Create parent directory
                if let Some(parent) = target_path.absolute_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                // Move the file
                // Get the path from directory_paths for this entry
                use crate::infra::db::entities::directory_paths;
                let dir_path = directory_paths::Entity::find_by_id(source_entry_id)
                    .one(db.conn())
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Directory path not found for entry"))?;

                let source_path = PathBuf::from(&dir_path.path);
                tokio::fs::rename(&source_path, &target_path.absolute_path).await?;

                // Update the sidecar record
                let mut active: sidecar::ActiveModel = sidecar.into();
                active.rel_path = ActiveValue::Set(target_path.relative_path.to_string_lossy().to_string());
                active.source_entry_id = ActiveValue::Set(None);
                active.source = ActiveValue::Set(Some("converted".to_string()));
                active.updated_at = ActiveValue::Set(Utc::now());
                active.update(db.conn()).await?;

                info!(
                    "Converted reference sidecar to owned: {} {} {} for {}",
                    kind.as_str(),
                    variant.as_str(),
                    format.as_str(),
                    content_uuid
                );
            }
        }

        Ok(())
    }

    /// Record sidecar creation
    pub async fn record_sidecar(
        &self,
        library: &Library,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        format: &SidecarFormat,
        size: u64,
        checksum: Option<String>,
    ) -> Result<()> {
        let db = library.db();
        let path = self.compute_path(&library.id(), content_uuid, kind, variant, format).await?;

        // Upsert sidecar record
        let sidecar = sidecar::ActiveModel {
            content_uuid: ActiveValue::Set(*content_uuid),
            kind: ActiveValue::Set(kind.as_str().to_string()),
            variant: ActiveValue::Set(variant.as_str().to_string()),
            format: ActiveValue::Set(format.as_str().to_string()),
            rel_path: ActiveValue::Set(path.relative_path.to_string_lossy().to_string()),
            size: ActiveValue::Set(size as i64),
            checksum: ActiveValue::Set(checksum.clone()),
            status: ActiveValue::Set("ready".to_string()),
            source: ActiveValue::Set(Some("sidecar_manager".to_string())),
            version: ActiveValue::Set(1),
            updated_at: ActiveValue::Set(Utc::now()),
            ..Default::default()
        };

        // Use upsert to handle both insert and update cases
        let result = Sidecar::find()
            .filter(sidecar::Column::ContentUuid.eq(*content_uuid))
            .filter(sidecar::Column::Kind.eq(kind.as_str()))
            .filter(sidecar::Column::Variant.eq(variant.as_str()))
            .one(db.conn())
            .await?;

        if let Some(existing) = result {
            let mut active: sidecar::ActiveModel = existing.into();
            active.rel_path = ActiveValue::Set(path.relative_path.to_string_lossy().to_string());
            active.size = ActiveValue::Set(size as i64);
            active.checksum = ActiveValue::Set(checksum);
            active.status = ActiveValue::Set("ready".to_string());
            active.updated_at = ActiveValue::Set(Utc::now());
            active.update(db.conn()).await?;
        } else {
            sidecar.insert(db.conn()).await?;
        }

        // Update local availability
        self.update_local_availability(
            library,
            content_uuid,
            kind,
            variant,
            true,
            Some(size),
            None,
        ).await?;

        Ok(())
    }

    /// Update local device availability
    async fn update_local_availability(
        &self,
        library: &Library,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
        has: bool,
        size: Option<u64>,
        checksum: Option<String>,
    ) -> Result<()> {
        let db = library.db();
        let device_uuid = self.context.device_manager.current_device().await.id;

        let availability = sidecar_availability::ActiveModel {
            content_uuid: ActiveValue::Set(*content_uuid),
            kind: ActiveValue::Set(kind.as_str().to_string()),
            variant: ActiveValue::Set(variant.as_str().to_string()),
            device_uuid: ActiveValue::Set(device_uuid),
            has: ActiveValue::Set(has),
            size: ActiveValue::Set(size.map(|s| s as i64)),
            checksum: ActiveValue::Set(checksum.clone()),
            last_seen_at: ActiveValue::Set(Utc::now()),
            ..Default::default()
        };

        // Try to update existing record
        let existing = SidecarAvailability::find()
            .filter(sidecar_availability::Column::ContentUuid.eq(*content_uuid))
            .filter(sidecar_availability::Column::Kind.eq(kind.as_str()))
            .filter(sidecar_availability::Column::Variant.eq(variant.as_str()))
            .filter(sidecar_availability::Column::DeviceUuid.eq(device_uuid))
            .one(db.conn())
            .await?;

        if let Some(existing) = existing {
            let mut active: sidecar_availability::ActiveModel = existing.into();
            active.has = ActiveValue::Set(has);
            active.size = ActiveValue::Set(size.map(|s| s as i64));
            active.checksum = ActiveValue::Set(checksum);
            active.last_seen_at = ActiveValue::Set(Utc::now());
            active.update(db.conn()).await?;
        } else {
            availability.insert(db.conn()).await?;
        }

        Ok(())
    }

    /// Remove a sidecar
    pub async fn remove_sidecar(
        &self,
        library: &Library,
        content_uuid: &Uuid,
        kind: &SidecarKind,
        variant: &SidecarVariant,
    ) -> Result<()> {
        let db = library.db();

        // Delete from database
        Sidecar::delete_many()
            .filter(sidecar::Column::ContentUuid.eq(*content_uuid))
            .filter(sidecar::Column::Kind.eq(kind.as_str()))
            .filter(sidecar::Column::Variant.eq(variant.as_str()))
            .exec(db.conn())
            .await?;

        // Update availability
        self.update_local_availability(
            library,
            content_uuid,
            kind,
            variant,
            false,
            None,
            None,
        ).await?;

        Ok(())
    }

    /// Bootstrap scan sidecars directory and sync with database
    pub async fn bootstrap_scan(&self, library: &Library) -> Result<()> {
        info!("Starting bootstrap scan for library {}", library.id());

        let builder = self.get_path_builder(&library.id()).await?;
        let sidecars_dir = builder.sidecars_dir();
        let content_dir = sidecars_dir.join("content");

        if !content_dir.exists() {
            info!("No sidecars directory found, skipping bootstrap scan");
            return Ok(());
        }

        let mut scanned_count = 0;

        // Walk through the sharded directory structure
        let mut shard_dirs = tokio::fs::read_dir(&content_dir).await?;

        while let Some(h0_entry) = shard_dirs.next_entry().await? {
            if !h0_entry.file_type().await?.is_dir() {
                continue;
            }

            let mut h0_dirs = tokio::fs::read_dir(h0_entry.path()).await?;

            while let Some(h1_entry) = h0_dirs.next_entry().await? {
                if !h1_entry.file_type().await?.is_dir() {
                    continue;
                }

                let mut content_dirs = tokio::fs::read_dir(h1_entry.path()).await?;

                while let Some(content_entry) = content_dirs.next_entry().await? {
                    if !content_entry.file_type().await?.is_dir() {
                        continue;
                    }

                    let content_uuid_str = content_entry.file_name();
                    let content_uuid = match Uuid::parse_str(&content_uuid_str.to_string_lossy()) {
                        Ok(uuid) => uuid,
                        Err(_) => {
                            warn!("Invalid content UUID directory: {:?}", content_uuid_str);
                            continue;
                        }
                    };

                    // Process all sidecars for this content
                    if let Err(e) = self.scan_content_sidecars(
                        library,
                        &content_uuid,
                        &content_entry.path(),
                    ).await {
                        error!("Failed to scan sidecars for {}: {}", content_uuid, e);
                    } else {
                        scanned_count += 1;
                    }
                }
            }
        }

        info!(
            "Bootstrap scan completed: scanned {} content directories",
            scanned_count
        );

        Ok(())
    }

    /// Scan all sidecars for a specific content UUID
    async fn scan_content_sidecars(
        &self,
        library: &Library,
        content_uuid: &Uuid,
        content_path: &Path,
    ) -> Result<()> {

        // Scan each sidecar kind directory
        for kind_str in ["thumbs", "proxies", "embeddings", "ocr", "transcript", "live_photos"] {
            let kind_path = content_path.join(kind_str);
            if !kind_path.exists() {
                continue;
            }

            let kind = match kind_str {
                "thumbs" => SidecarKind::Thumb,
                "proxies" => SidecarKind::Proxy,
                "embeddings" => SidecarKind::Embeddings,
                "ocr" => SidecarKind::Ocr,
                "transcript" => SidecarKind::Transcript,
                "live_photos" => SidecarKind::LivePhotoVideo,
                _ => continue,
            };

            let mut entries = tokio::fs::read_dir(&kind_path).await?;

            while let Some(entry) = entries.next_entry().await? {
                if !entry.file_type().await?.is_file() {
                    continue;
                }

                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                // Parse variant and format from filename
                if let Some((variant_str, format_str)) = file_name_str.rsplit_once('.') {
                    let variant = SidecarVariant::new(variant_str);
                    let format = match format_str.try_into() {
                        Ok(f) => f,
                        Err(_) => {
                            warn!("Unknown sidecar format: {}", format_str);
                            continue;
                        }
                    };

                    // Get file metadata
                    let metadata = entry.metadata().await?;
                    let size = metadata.len();

                    // Record in database
                    self.record_sidecar(
                        library,
                        content_uuid,
                        &kind,
                        &variant,
                        &format,
                        size,
                        None, // TODO: Compute checksum if needed
                    ).await?;
                }
            }
        }

        Ok(())
    }

    /// Start watching the sidecars directory for changes
    pub async fn start_watcher(&self, library: &Library) -> Result<()> {
        // TODO: Implement filesystem watcher
        // This would integrate with the existing location_watcher service
        // to monitor changes in the sidecars directory
        warn!("Sidecar filesystem watcher not yet implemented");
        Ok(())
    }
}

/// Result of a sidecar request
#[derive(Debug, Clone)]
pub enum SidecarResult {
    /// Sidecar is ready with relative path
    Ready(PathBuf),
    /// Sidecar is being generated
    Pending,
}

/// Sidecar presence information
#[derive(Debug, Clone)]
pub struct SidecarPresence {
    /// Available locally
    pub local: bool,
    /// Local path if available
    pub path: Option<PathBuf>,
    /// Current status
    pub status: SidecarStatus,
    /// Remote devices that have this sidecar
    pub devices: Vec<Uuid>,
}