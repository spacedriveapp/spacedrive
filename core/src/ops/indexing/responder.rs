//! Change Detection Responder (function-style)
//!
//! Consumes raw watcher events and applies DB-backed changes using the indexing module.

use crate::context::CoreContext;
use crate::infra::event::{Event, FsRawEventKind};
use crate::ops::indexing::change_detection::{Change, ChangeDetector};
use crate::ops::indexing::entry::EntryProcessor;
use crate::ops::indexing::state::{DirEntry, EntryKind, IndexerState};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

/// Apply a raw FS change by resolving it to a DB-backed operation and emitting canonical events
pub async fn apply(context: &Arc<CoreContext>, library_id: Uuid, kind: FsRawEventKind) -> Result<()> {
    match kind {
        FsRawEventKind::Create { path } => {
            debug!("Create: {}", path.display());
            // TODO: resolve parent, create entry via EntryProcessor, emit EntryCreated
        }
        FsRawEventKind::Modify { path } => {
            debug!("Modify: {}", path.display());
            // TODO: resolve entry id, update entry, emit EntryModified
        }
        FsRawEventKind::Remove { path } => {
            debug!("Remove: {}", path.display());
            // TODO: resolve entry id, delete subtree, emit EntryDeleted
        }
        FsRawEventKind::Rename { from, to } => {
            debug!("Rename: {} -> {}", from.display(), to.display());
            // TODO: resolve entry id, move entry, update directory_paths descendants, emit EntryMoved
        }
    }
    Ok(())
}

