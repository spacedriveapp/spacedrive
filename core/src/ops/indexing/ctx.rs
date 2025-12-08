//! Context abstraction for indexing operations.
//!
//! The `IndexingCtx` trait provides a minimal interface that indexing code paths
//! need to function. This allows the same indexing logic to run both inside the
//! job system (with `JobContext`) and outside of it (watcher responder), avoiding
//! code duplication between job-based and event-driven indexing.

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::{context::CoreContext, infra::job::prelude::JobContext, library::Library};

/// Minimal interface required by indexing operations.
///
/// This trait abstracts away the difference between job-based indexing and
/// event-driven indexing (file watcher responders). Both execution contexts
/// provide database access and logging, but only the job context has full
/// library access for sync operations.
pub trait IndexingCtx {
	fn library_db(&self) -> &DatabaseConnection;

	/// Returns the library reference when running in job context, None otherwise.
	///
	/// This is only available for job-based indexing since responder contexts
	/// don't have direct library access (they operate through the event bus).
	fn library(&self) -> Option<&Library> {
		None
	}

	fn log(&self, message: impl AsRef<str>) {
		tracing::debug!(message = %message.as_ref());
	}
}

impl<'a> IndexingCtx for JobContext<'a> {
	fn library_db(&self) -> &DatabaseConnection {
		self.library_db()
	}

	fn library(&self) -> Option<&Library> {
		Some(self.library())
	}
}

/// Context for file watcher responders that run outside the job system.
///
/// Responders handle filesystem events (file created, moved, deleted) by
/// performing incremental indexing updates. They operate independently of
/// the job system and communicate results through the event bus rather than
/// job completion.
pub struct ResponderCtx {
	db: DatabaseConnection,
}

impl ResponderCtx {
	pub async fn new(context: &Arc<CoreContext>, library_id: Uuid) -> anyhow::Result<Self> {
		let library: Arc<Library> = context
			.get_library(library_id)
			.await
			.ok_or_else(|| anyhow::anyhow!("Library not found: {}", library_id))?;

		Ok(Self {
			db: library.db().conn().clone(),
		})
	}
}

impl IndexingCtx for ResponderCtx {
	fn library_db(&self) -> &DatabaseConnection {
		&self.db
	}
}
