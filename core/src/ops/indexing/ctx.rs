//! Lightweight context abstraction for indexing operations
//!
//! Provides a minimal interface required by indexing code paths so they can run
//! either inside the job system (with `JobContext`) or outside of it (watcher
//! responder) without duplicating logic.

use sea_orm::DatabaseConnection;
use std::sync::Arc;
use uuid::Uuid;

use crate::{context::CoreContext, infra::job::prelude::JobContext, library::Library};

/// Minimal capabilities needed by indexing operations
pub trait IndexingCtx {
	/// Access to the library database connection
	fn library_db(&self) -> &DatabaseConnection;

	/// Lightweight logging hook
	fn log(&self, message: impl AsRef<str>) {
		tracing::debug!(message = %message.as_ref());
	}
}

impl<'a> IndexingCtx for JobContext<'a> {
	fn library_db(&self) -> &DatabaseConnection {
		self.library_db()
	}
}

/// Context for responder paths running outside the job system
pub struct ResponderCtx {
	/// Cloned DB connection for the target library
	db: DatabaseConnection,
}

impl ResponderCtx {
	/// Build a responder context for a specific library
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
