//! Change Detection Responder
//!
//! Bridges raw watcher events to real database-backed entry events.

use crate::infra::event::{Event, EventBus, FsRawEventKind};
use crate::ops::indexing::change_detection::{Change, ChangeDetector};
use crate::ops::indexing::entry::EntryProcessor;
use crate::ops::indexing::state::{DirEntry, EntryKind, IndexMode, IndexerState};
use crate::ops::indexing::PathResolver;
use crate::{infra::job::prelude::JobContext, service::Service};
use anyhow::Result;
use sea_orm::TransactionTrait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Listens for FsRawChange events and resolves them into database-backed Entry events
pub struct ChangeDetectionResponder {
    events: Arc<EventBus>,
}

impl ChangeDetectionResponder {
    pub fn new(events: Arc<EventBus>) -> Self {
        Self { events }
    }

    /// Start the responder loop
    pub async fn start(&self) -> Result<()> {
        let mut subscriber = self.events.subscribe();
        let events = self.events.clone();

        tokio::spawn(async move {
            loop {
                match subscriber.recv().await {
                    Ok(Event::FsRawChange { library_id, kind }) => {
                        if let Err(e) = Self::handle_raw_change(&events, library_id, kind).await {
                            error!("Responder failed for FsRawChange: {}", e);
                        }
                    }
                    Ok(_) => {
                        // ignore other events
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        debug!("Responder lagged; continuing");
                    }
                }
            }
        });

        Ok(())
    }

    async fn handle_raw_change(
        events: &Arc<EventBus>,
        library_id: Uuid,
        kind: FsRawEventKind,
    ) -> Result<()> {
        // Obtain the library and DB via global context is outside this module; for now this
        // responder only emits raw-to-core conversions once DB updates are applied by index jobs.
        // Minimal implementation: emit core entry events only after resolving to DB where possible.

        match kind {
            FsRawEventKind::Create { path } => {
                // Defer to indexing job: we cannot resolve parent reliably without state here.
                // In a full integration, we'd query directory_paths for parent and create entry.
                // For now, no-op to avoid emitting synthetic IDs.
                debug!("FsRawCreate observed: {}", path.display());
            }
            FsRawEventKind::Modify { path } => {
                debug!("FsRawModify observed: {}", path.display());
            }
            FsRawEventKind::Remove { path } => {
                debug!("FsRawRemove observed: {}", path.display());
            }
            FsRawEventKind::Rename { from, to } => {
                debug!("FsRawRename observed: {} -> {}", from.display(), to.display());
            }
        }

        Ok(())
    }
}

